//! The tree pane: its chrome, its rows from lines the host lays out, and
//! its scrollbar.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use super::TreeRow;
use super::state::TreeState;
use crate::scroll::{ScrollGutter, ScrollWindow, WrappedRows};
use crate::selection::{self, Cursor};
use crate::text;
use crate::theme::Theme;

/// A mark the host puts on some of its rows, such as a bookmark: on while
/// what the row stands for is live, off while it is not. The tree hands it
/// to the row's producer, which draws it; it is not the cursor's marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RowMark {
    /// The row carries no mark.
    #[default]
    None,
    /// The row is marked, and what it stands for is live.
    On,
    /// The row is marked, and what it stands for is not live.
    Off,
}

/// What the host knows about a row that the tree draws: worked out before
/// the frame, so drawing a row looks nothing up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RowState {
    /// The row stands for something gone, stopped, or not started: it
    /// wears the ghost look ([`selection::ghost`]).
    pub ghost: bool,
    /// The row keeps old values while a change it waits for lands: it
    /// recedes ([`selection::pending`]), and its producer may show a busy
    /// mark in place of a state it cannot vouch for.
    pub pending: bool,
    /// The host's mark for the row.
    pub mark: RowMark,
}

/// Everything a row's producer is told about the row it lays out, beside
/// the row itself.
///
/// The producer writes the row's cells at [`width`](Self::width); the tree
/// then applies the looks the context names ([`apply_looks`](Self::apply_looks)):
/// the pending and ghost looks, then the cursor's highlight.
#[derive(Clone, Copy, Debug)]
pub struct RowCtx<'a> {
    /// The cursor is on the row: the row has the selected identity.
    pub selected: bool,
    /// The tree has focus, so the cursor row takes the active highlight.
    pub focused: bool,
    /// The row is expanded (its children show), when it has children.
    pub expanded: bool,
    /// The row wears the ghost look, as its [`RowState`] says.
    pub ghost: bool,
    /// The row recedes while a change lands, as its [`RowState`] says.
    pub pending: bool,
    /// The host's mark for the row.
    pub mark: RowMark,
    /// The cells the row may take: the body's width, less the scrollbar
    /// gutter when the rows overflow.
    pub width: usize,
    /// The theme the tree draws with.
    pub theme: &'a Theme,
}

impl RowCtx<'_> {
    /// The cursor on the row: off, active (focused), or inactive.
    pub const fn cursor(&self) -> Cursor {
        Cursor::of(self.selected, self.focused)
    }

    /// Apply the looks the context names to a row's spans, in the order
    /// that keeps each visible: the pending look, then the ghost look, then
    /// the cursor's highlight (so a selected ghost row is still a ghost).
    /// The tree does this to every line a producer returns; a host that
    /// draws one row on its own calls it too.
    pub fn apply_looks(&self, spans: &mut [Span<'_>]) {
        if self.pending {
            selection::pending(spans, self.theme);
        }
        if self.ghost {
            selection::ghost(spans, self.theme);
        }
        selection::highlight(spans, self.cursor(), self.theme);
    }
}

/// Inside-border rows the pane needs before it shows its footer banner.
const FOOTER_ROOM: u16 = 4;
/// The most rows a footer banner takes.
const FOOTER_ROWS: usize = 3;
/// Inside-border rows the pane needs before it shows its header line.
const HEADER_ROOM: u16 = 3;

/// A tree in a bordered pane: a title on the top border, an optional
/// header line, the showing rows in a window that follows the cursor, the
/// scrollbar gutter, an optional footer banner, and a placeholder when no
/// row shows.
///
/// The host lays each row out: the producer `line` takes the row and its
/// [`RowCtx`] and returns the row's [`Line`], at most `width` cells. The
/// tree decides which rows show, which of them the window shows, and how
/// the cursor row and the host's [`RowState`]s look. Only the rows the
/// window shows are laid out.
///
/// [`render`](Self::render) returns the window it drew, which the host
/// keeps with [`TreeState::set_viewport`], or `None` when no row showed
/// (the kept window stays for when rows come back).
pub struct TreeView<'a, R: TreeRow, F> {
    rows: &'a [R],
    state: &'a TreeState<R::Id>,
    line: F,
    theme: &'a Theme,
    focused: bool,
    states: &'a [RowState],
    title: Option<Line<'a>>,
    header: Option<Line<'a>>,
    footer: Option<(Vec<Line<'a>>, Style)>,
    empty: Option<String>,
}

impl<'a, R, F> TreeView<'a, R, F>
where
    R: TreeRow,
    F: Fn(&R, &RowCtx<'_>) -> Line<'static>,
{
    /// A pane over `rows` in `state`, each row laid out by `line`. It has
    /// focus, no title, no header, no footer, no placeholder, and no row
    /// states until the builder methods say otherwise.
    pub const fn new(
        rows: &'a [R],
        state: &'a TreeState<R::Id>,
        line: F,
        theme: &'a Theme,
    ) -> Self {
        Self {
            rows,
            state,
            line,
            theme,
            focused: true,
            states: &[],
            title: None,
            header: None,
            footer: None,
            empty: None,
        }
    }

    /// Whether the tree has focus: its cursor row takes the active
    /// highlight with focus and the inactive one without.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// What the host knows about each row, index for index with the rows
    /// (a row past the end of `states` has the default state).
    #[must_use]
    pub const fn states(mut self, states: &'a [RowState]) -> Self {
        self.states = states;
        self
    }

    /// The title drawn on the top border, such as a strip of tabs. The
    /// host lays it out for the border's width, the pane's less two.
    #[must_use]
    pub fn title(mut self, title: Line<'a>) -> Self {
        self.title = Some(title);
        self
    }

    /// A line above the rows, such as what the rows are a listing of.
    /// Shown when the pane has more than three rows inside its border.
    #[must_use]
    pub fn header(mut self, header: Line<'a>) -> Self {
        self.header = Some(header);
        self
    }

    /// A banner under the rows, such as advice about the data: the lines
    /// wrapped to the pane's width, at most three rows, drawn in `style`.
    /// Shown when the pane has more than four rows inside its border.
    #[must_use]
    pub fn footer(mut self, lines: Vec<Line<'a>>, style: Style) -> Self {
        self.footer = Some((lines, style));
        self
    }

    /// What the pane says when no row shows, wrapped to its width in the
    /// muted look.
    #[must_use]
    pub fn empty(mut self, text: impl Into<String>) -> Self {
        self.empty = Some(text.into());
        self
    }

    /// Draw the pane in `area` and return the window its rows drew, or
    /// `None` when no row showed.
    pub fn render(mut self, area: Rect, buf: &mut Buffer) -> Option<ScrollWindow> {
        let mut block = Block::default().borders(Borders::ALL);
        if let Some(title) = self.title.take() {
            block = block.title(title);
        }
        let mut inner = block.inner(area);
        block.render(area, buf);

        if let Some((lines, style)) = self.footer.take()
            && inner.height > FOOTER_ROOM
        {
            let rows = WrappedRows::new(&lines, usize::from(inner.width));
            let height = u16::try_from(rows.len().clamp(1, FOOTER_ROWS)).unwrap_or(1);
            let banner = Rect {
                y: inner.y + inner.height - height,
                height,
                ..inner
            };
            Paragraph::new(rows.into_rows())
                .style(style)
                .render(banner, buf);
            inner.height -= height;
        }

        if let Some(header) = self.header.take()
            && inner.height > HEADER_ROOM
        {
            Paragraph::new(header).render(Rect { height: 1, ..inner }, buf);
            inner.y += 1;
            inner.height -= 1;
        }

        let shown = self.state.visible_indices(self.rows);
        if shown.is_empty() {
            if let Some(empty) = self.empty.take() {
                let lines: Vec<Line<'static>> = text::wrap(&empty, usize::from(inner.width))
                    .into_iter()
                    .map(Line::from)
                    .collect();
                Paragraph::new(lines)
                    .style(Style::default().add_modifier(self.theme.muted))
                    .render(inner, buf);
            }
            return None;
        }

        // The gutter is carved out of the body when the rows overflow, so
        // rows are laid out at the width that is left.
        let gutter = ScrollGutter::layout(inner, shown.len());
        let width = usize::from(gutter.content().width);
        let window = self
            .state
            .window_over(self.rows, &shown, usize::from(inner.height));
        let lines: Vec<Line<'static>> = shown[window.visible()]
            .iter()
            .map(|&index| self.row_line(index, width))
            .collect();
        Paragraph::new(lines).render(gutter.content(), buf);
        gutter.render(&window, buf);
        Some(window)
    }

    /// The line of the row at `index` in the rows, laid out by the producer
    /// at `width` and given its looks.
    fn row_line(&self, index: usize, width: usize) -> Line<'static> {
        let row = &self.rows[index];
        let state = self.states.get(index).copied().unwrap_or_default();
        let cx = RowCtx {
            selected: self.state.selected() == Some(row.id()),
            focused: self.focused,
            expanded: self.state.is_expanded(row.id()),
            ghost: state.ghost,
            pending: state.pending,
            mark: state.mark,
            width,
            theme: self.theme,
        };
        let mut line = (self.line)(row, &cx);
        cx.apply_looks(&mut line.spans);
        line
    }
}
