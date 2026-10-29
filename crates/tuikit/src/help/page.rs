//! The help page: its layout in cells, the frame it draws in, and where
//! it is scrolled.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::content::{HelpRow, HelpSection};
use crate::frame::{ModalBody, ModalFrame, ModalSize};
use crate::keys::{Footer, KeyRow, PagerKey, unavailable_style};
use crate::scroll::{ScrollGutter, ScrollWindow};
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
/// Pages this narrow, or narrower, leave the footer on one row.
const MIN_FOOTER: usize = 8;

/// A help page: its sections laid out for the frame's width, scrolled,
/// with a footer that stays on the frame's last rows.
///
/// The page draws in a [`ModalFrame`] at [`HelpPage::SIZE`] titled
/// `Help`. Its layout, at any width:
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
/// - **The footer** generated from the key table that scrolls and closes
///   the page ([`footer`](Self::footer)), in the theme's `muted` look and
///   wrapped only between its segments. It stays on the last rows whatever
///   the scroll, leaving at least one row to the rest; footer rows that do
///   not fit there scroll with the page.
/// - **A scrollbar** in a 1-column gutter when the page is taller than the
///   frame, the text laid out one cell narrower to make room for it.
///
/// Drawing returns the window the scrolling part showed; keep it in a
/// [`HelpScroll`] so the next page key moves by the rows on screen and an
/// offset past the end comes back.
#[derive(Clone, Debug)]
pub struct HelpPage<'a> {
    theme: &'a Theme,
    sections: &'a [HelpSection],
    title: Line<'a>,
    footer: Footer,
    offset: usize,
}

impl<'a> HelpPage<'a> {
    /// How large a help page is: a reader's size, as wide as the screen
    /// allows up to 100 columns and as tall as it allows.
    pub const SIZE: ModalSize = ModalSize::WIDE;

    /// A page of `sections`, titled `Help`, at the top, with no footer.
    pub fn new(theme: &'a Theme, sections: &'a [HelpSection]) -> Self {
        Self {
            theme,
            sections,
            title: Line::from("Help"),
            footer: Footer::default(),
            offset: 0,
        }
    }

    /// Sets the title.
    #[must_use]
    pub fn title(mut self, title: impl Into<Line<'a>>) -> Self {
        self.title = title.into();
        self
    }

    /// Generates the footer from `table`, the keys that scroll and close
    /// the page.
    #[must_use]
    pub fn footer<A>(mut self, table: &[KeyRow<A>]) -> Self {
        self.footer = Footer::new(table);
        self
    }

    /// Scrolls the page to where `scroll` is.
    #[must_use]
    pub fn scrolled(mut self, scroll: &HelpScroll) -> Self {
        self.offset = scroll.offset();
        self
    }

    /// The sections laid out `width` cells wide, as the page draws them
    /// above its footer. A width of `usize::MAX` wraps nothing.
    pub fn lines(&self, width: usize) -> Vec<Line<'static>> {
        let mut out = Vec::new();
        for section in self.sections {
            section_lines(section, self.theme, width, &mut out);
        }
        out
    }

    /// The footer laid out `width` cells wide, in its `muted` look.
    pub fn footer_lines(&self, width: usize) -> Vec<Line<'static>> {
        if self.footer.is_empty() {
            return Vec::new();
        }
        let width = if width <= MIN_FOOTER {
            usize::MAX
        } else {
            width
        };
        let muted = Style::default().add_modifier(self.theme.muted);
        self.footer
            .lines(width, self.theme)
            .into_iter()
            .map(|line| restyle(line, |style| muted.patch(style)))
            .collect()
    }

    /// Draws the page in its frame over `area` and returns the window its
    /// scrolling part showed, the offset brought inside the page.
    pub fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let frame = ModalFrame::new(self.theme, Self::SIZE).title(self.title.clone());
        frame.render(area, buf, Body { page: &self })
    }

    /// The page split, at `width` by `height` cells, into what scrolls and
    /// the footer rows that stay.
    fn layout(&self, width: usize, height: usize) -> Layout {
        let split = |text_width: usize| {
            let mut body = self.lines(text_width);
            let mut footer = self.footer_lines(text_width);
            let stay = footer.len().min(height.saturating_sub(1));
            let spill = footer.len() - stay;
            body.extend(footer.drain(..spill));
            Layout {
                body_height: height - stay,
                body,
                footer,
                text_width,
            }
        };
        let page = split(width);
        if page.body.len() <= page.body_height || width <= 1 {
            return page;
        }
        // The scrollbar takes a column once the page overflows.
        split(width - 1)
    }
}

/// A page laid out for its frame.
struct Layout {
    /// The rows that scroll.
    body: Vec<Line<'static>>,
    /// The rows that stay at the bottom.
    footer: Vec<Line<'static>>,
    /// Rows the scrolling part has.
    body_height: usize,
    /// Columns the text has: one fewer than the frame's when a scrollbar
    /// is drawn.
    text_width: usize,
}

/// The page inside its frame.
struct Body<'p, 'a> {
    page: &'p HelpPage<'a>,
}

impl ModalBody for Body<'_, '_> {
    type Output = ScrollWindow;

    /// A help page fills its frame, so it is laid out in `render`, where
    /// the height is known.
    fn measure(&mut self, _width: u16) -> u16 {
        0
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let layout = self
            .page
            .layout(usize::from(area.width), usize::from(area.height));
        let body_height = u16::try_from(layout.body_height).unwrap_or(area.height);
        let window = ScrollWindow::new(layout.body_height, layout.body.len()).at(self.page.offset);
        let gutter = ScrollGutter::layout(
            Rect {
                height: body_height,
                ..area
            },
            layout.body.len(),
        );
        let shown = layout.body[window.visible()].to_vec();
        Paragraph::new(shown).render(gutter.content(), buf);
        let footer_area = Rect {
            y: area.y + body_height,
            width: u16::try_from(layout.text_width).unwrap_or(area.width),
            height: area.height - body_height,
            ..area
        };
        Paragraph::new(layout.footer).render(footer_area, buf);
        gutter.render(&window, buf);
        window
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

/// Where a help page is scrolled, kept between draws.
///
/// A step moves the offset the page asks for; a step past the end is
/// allowed, so the end key works before the page has been drawn. Each
/// draw shows the page at that offset brought inside it and returns the
/// window it showed; [`keep`](Self::keep) takes the window back, so the
/// offset returns inside the page at once (one key up after the end
/// scrolls up, however many keys went past it) and a page key moves by
/// the rows the page showed (house rule 12).
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use tuikit::help::{HelpPage, HelpScroll, HelpSection};
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let mut section = HelpSection::new("Rows");
/// for n in 0..40 {
///     section = section.keys(format!("{n}"), "a row");
/// }
/// let sections = [section];
/// let area = Rect::new(0, 0, 60, 16);
/// let mut scroll = HelpScroll::new();
///
/// scroll.to_bottom();
/// let window = HelpPage::new(&theme, &sections)
///     .scrolled(&scroll)
///     .render(area, &mut Buffer::empty(area));
/// scroll.keep(window);
/// assert_eq!(scroll.offset(), window.last_offset(), "the end, not past it");
/// assert_eq!(scroll.page(), window.viewport(), "a page is the rows shown");
///
/// scroll.up(1);
/// assert_eq!(scroll.offset(), window.last_offset() - 1);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HelpScroll {
    offset: usize,
    page: usize,
}

impl HelpScroll {
    /// At the top, before any draw.
    pub const fn new() -> Self {
        Self { offset: 0, page: 0 }
    }

    /// Scrolled to `offset`, before any draw.
    pub const fn at(offset: usize) -> Self {
        Self { offset, page: 0 }
    }

    /// The first row the page asks to show. It may pass the end until the
    /// next draw's window is kept.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// How far a page key moves: the rows the last draw showed, at least
    /// one.
    pub const fn page(&self) -> usize {
        if self.page == 0 { 1 } else { self.page }
    }

    /// Scrolls down `rows` rows.
    pub fn down(&mut self, rows: usize) {
        self.offset = self.offset.saturating_add(rows);
    }

    /// Scrolls up `rows` rows, stopping at the top.
    pub fn up(&mut self, rows: usize) {
        self.offset = self.offset.saturating_sub(rows);
    }

    /// Scrolls down a [`page`](Self::page).
    pub fn page_down(&mut self) {
        self.down(self.page());
    }

    /// Scrolls up a [`page`](Self::page).
    pub fn page_up(&mut self) {
        self.up(self.page());
    }

    /// Scrolls down half a [`page`](Self::page), at least one row.
    pub fn half_page_down(&mut self) {
        self.down((self.page() / 2).max(1));
    }

    /// Scrolls up half a [`page`](Self::page), at least one row.
    pub fn half_page_up(&mut self) {
        self.up((self.page() / 2).max(1));
    }

    /// Scrolls to the first row.
    pub fn to_top(&mut self) {
        self.offset = 0;
    }

    /// Scrolls to the end: the next draw shows the last page.
    pub fn to_bottom(&mut self) {
        self.offset = usize::MAX;
    }

    /// Does what a standard pager key asks: the scroll keys move the page
    /// and return `true`; the keys that leave, find, open help, or copy
    /// are the control's and return `false`.
    pub fn apply(&mut self, key: PagerKey) -> bool {
        match key {
            PagerKey::Down => self.down(1),
            PagerKey::Up => self.up(1),
            PagerKey::PageDown => self.page_down(),
            PagerKey::PageUp => self.page_up(),
            PagerKey::HalfPageDown => self.half_page_down(),
            PagerKey::HalfPageUp => self.half_page_up(),
            PagerKey::Top => self.to_top(),
            PagerKey::Bottom => self.to_bottom(),
            PagerKey::Abort
            | PagerKey::Close
            | PagerKey::Find
            | PagerKey::FindNext
            | PagerKey::FindPrev
            | PagerKey::Help
            | PagerKey::Copy => return false,
        }
        true
    }

    /// Takes in the window a draw showed: the offset comes back inside the
    /// page, and a page key moves by the rows on screen.
    pub fn keep(&mut self, window: ScrollWindow) {
        self.offset = window.offset();
        self.page = window.viewport();
    }
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
