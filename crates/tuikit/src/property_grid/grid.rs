//! The grid: rows in order, lines that are no row between them, the one
//! layout every row shares, and the intents a row reports.

use ratatui::text::{Line, Span};

use super::{DisclosureRow, LABEL_WIDTH, LinkRow, PropertyRow, Tones, VERB_WIDTH};
use crate::selection::{self, Cursor};
use crate::theme::Theme;

/// What a row asks its host to do. The grid never does it: the host
/// writes a copy to its clipboard sink, opens an inspected value in a
/// reader, folds a section, or moves to a link's far end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Put `text`, a property's whole value, on the clipboard; `label`
    /// names it (for the note that says what was copied).
    Copy {
        /// The property's label.
        label: String,
        /// The whole value.
        text: String,
    },
    /// Show `text`, a value the row draws cut, whole in a reader titled
    /// `label`.
    Inspect {
        /// The property's label.
        label: String,
        /// The whole value.
        text: String,
    },
    /// Open or close the section the header row heads.
    Toggle,
    /// Follow the link to its far end.
    Follow,
}

/// One row of a grid: a property, a link, or a section header.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// A label and a value.
    Property(PropertyRow),
    /// Where a link leads.
    Link(LinkRow),
    /// A section header that opens and closes the rows under it.
    Disclosure(DisclosureRow),
}

impl Row {
    /// What `Enter` on the row asks for: a property's copy, a link's
    /// follow, a header's toggle, or nothing.
    pub fn activate(&self) -> Option<Intent> {
        match self {
            Self::Property(row) => row.activate(),
            Self::Link(row) => row.activate(),
            Self::Disclosure(row) => row.activate(),
        }
    }

    /// What the inspect key on the row asks for: a cut property's whole
    /// value, or nothing.
    pub fn inspect(&self) -> Option<Intent> {
        match self {
            Self::Property(row) => row.inspect(),
            Self::Link(_) | Self::Disclosure(_) => None,
        }
    }

    fn draw(&self, layout: &Layout<'_>, width: usize, theme: &Theme) -> Vec<Span<'static>> {
        match self {
            Self::Property(row) => row.draw(layout, theme),
            Self::Link(row) => row.draw(layout, width),
            Self::Disclosure(row) => row.draw(theme),
        }
    }
}

impl From<PropertyRow> for Row {
    fn from(row: PropertyRow) -> Self {
        Self::Property(row)
    }
}

impl From<LinkRow> for Row {
    fn from(row: LinkRow) -> Self {
        Self::Link(row)
    }
}

impl From<DisclosureRow> for Row {
    fn from(row: DisclosureRow) -> Self {
        Self::Disclosure(row)
    }
}

/// An entry of the grid: a row the cursor visits, or a line it skips.
#[derive(Clone, Debug, PartialEq)]
enum Item {
    Row(Row),
    Line(Line<'static>),
}

/// The layout every row of one grid shares.
pub(super) struct Layout<'a> {
    /// The label column's cells.
    pub(super) label_width: usize,
    /// The verb column's cells.
    pub(super) verb_width: usize,
    /// The key the truncated hint names.
    pub(super) inspect_key: Option<&'a str>,
    /// The look of each link tone.
    pub(super) tones: Tones,
}

/// A grid's lines as drawn, and which of them is the cursor's row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridLines {
    /// Every row and every line that is no row, in order.
    pub lines: Vec<Line<'static>>,
    /// The index in [`GridLines::lines`] of the cursor's row, when the
    /// cursor is on one.
    pub cursor_line: Option<usize>,
}

/// Rows of properties, links, and section headers in one layout, with
/// lines that are no row between them.
///
/// Every row starts with the cursor gutter and is highlighted whole on
/// the cursor ([`selection::row`]), in the active look while the grid
/// has focus and the inactive one while it does not. A line pushed with
/// [`PropertyGrid::push_line`] (a rule, a heading) is drawn as it is and
/// is not counted: the cursor and the indices [`PropertyGrid::activate`]
/// and [`PropertyGrid::inspect`] take count rows only.
///
/// Property rows and section headers are drawn whole, and a host that
/// wraps the lines to a narrow area wraps a long value; link rows are
/// laid out in the width [`PropertyGrid::lines`] is given and never
/// wrap.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyGrid {
    items: Vec<Item>,
    label_width: usize,
    verb_width: usize,
    inspect_key: Option<String>,
    tones: Option<Tones>,
}

impl Default for PropertyGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl PropertyGrid {
    /// An empty grid with a [`LABEL_WIDTH`] label column and a
    /// [`VERB_WIDTH`] verb column, its link tones from the theme it is
    /// drawn with.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            label_width: LABEL_WIDTH,
            verb_width: VERB_WIDTH,
            inspect_key: None,
            tones: None,
        }
    }

    /// A label column of `cells`.
    pub fn label_width(mut self, cells: usize) -> Self {
        self.label_width = cells;
        self
    }

    /// A verb column of `cells`.
    pub fn verb_width(mut self, cells: usize) -> Self {
        self.verb_width = cells;
        self
    }

    /// The label of the key the host binds to [`PropertyGrid::inspect`],
    /// which a cut value's hint names (`(truncated · o)`). Without one the
    /// hint says only `(truncated)`.
    pub fn inspect_key(mut self, key: impl Into<String>) -> Self {
        self.inspect_key = Some(key.into());
        self
    }

    /// The look of each link tone, in place of the theme's tokens.
    pub fn tones(mut self, tones: Tones) -> Self {
        self.tones = Some(tones);
        self
    }

    /// The grid with `row` added after its rows.
    pub fn row(mut self, row: impl Into<Row>) -> Self {
        self.push(row);
        self
    }

    /// Adds `row` after the grid's rows.
    pub fn push(&mut self, row: impl Into<Row>) {
        self.items.push(Item::Row(row.into()));
    }

    /// Adds a line that is no row (a rule, a heading): drawn as it is,
    /// with no gutter, and skipped by the cursor.
    pub fn push_line(&mut self, line: Line<'static>) {
        self.items.push(Item::Line(line));
    }

    /// How many rows the grid holds, lines that are no row left out.
    pub fn len(&self) -> usize {
        self.rows().count()
    }

    /// Whether the grid holds no row.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The row at `index`, counting rows only.
    pub fn get(&self, index: usize) -> Option<&Row> {
        self.rows().nth(index)
    }

    /// What `Enter` on the row at `index` asks for.
    pub fn activate(&self, index: usize) -> Option<Intent> {
        self.get(index).and_then(Row::activate)
    }

    /// What the inspect key on the row at `index` asks for.
    pub fn inspect(&self, index: usize) -> Option<Intent> {
        self.get(index).and_then(Row::inspect)
    }

    /// Every row and line, laid out in `width` cells, with the cursor on
    /// the row at `cursor` (none when `None`) in the look of a grid that
    /// is `focused` or not.
    pub fn lines(
        &self,
        width: usize,
        cursor: Option<usize>,
        focused: bool,
        theme: &Theme,
    ) -> GridLines {
        let layout = Layout {
            label_width: self.label_width,
            verb_width: self.verb_width,
            inspect_key: self.inspect_key.as_deref(),
            tones: self.tones.unwrap_or_else(|| Tones::new(theme)),
        };
        let mut drawn = GridLines::default();
        let mut index = 0;
        for item in &self.items {
            match item {
                Item::Line(line) => drawn.lines.push(line.clone()),
                Item::Row(row) => {
                    let on = cursor == Some(index);
                    if on {
                        drawn.cursor_line = Some(drawn.lines.len());
                    }
                    drawn.lines.push(selection::row(
                        row.draw(&layout, width, theme),
                        Cursor::of(on, focused),
                        theme,
                    ));
                    index += 1;
                }
            }
        }
        drawn
    }

    fn rows(&self) -> impl Iterator<Item = &Row> {
        self.items.iter().filter_map(|item| match item {
            Item::Row(row) => Some(row),
            Item::Line(_) => None,
        })
    }
}
