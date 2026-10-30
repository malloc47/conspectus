//! The declaration: columns, their widths, and how each gives way.

use super::{GAP, MIN_FILL};

/// How wide a column is before anything gives way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    /// Always this many cells, whatever the rows hold. A column of marks
    /// that every row draws, an age, a chip.
    Fixed(usize),
    /// As wide as the widest cell under the column's key over every row
    /// measured ([`Measure`](super::Measure)), at most this many cells
    /// (`usize::MAX` for no bound); a wider cell counts as this many and
    /// is cut to them. A column whose cells are all blank
    /// takes no cells and no gap, so a mark only some trees have is
    /// reserved on every row of a tree that has one, and nowhere else.
    Cap(usize),
    /// The cells of the first step whose room the column reaches:
    /// `(from, cells)` pairs, widest first (`&[(100, 30), (60, 24), (0,
    /// 18)]`). The room counts from the column to the row's end, so the
    /// columns before it are already taken off.
    Steps(&'static [(usize, usize)]),
    /// The cells of the first step whose width the whole row reaches, its
    /// lead included: `(from, cells)` pairs, widest first, like
    /// [`Steps`](Self::Steps), but whatever the columns before it take, so
    /// the column keeps its width when one of them gives way, and a list
    /// that counts a cursor gutter or an indent as its lead steps where a
    /// list without one of that whole width does.
    RowSteps(&'static [(usize, usize)]),
    /// `per` of every `of` cells of the whole row, its lead included,
    /// rounded down and kept between `min` and `max`: a label that grows
    /// with the row. Two lists of one whole width give the column one
    /// width, whatever each draws before its schema.
    Share {
        /// The cells taken of every `of`.
        per: usize,
        /// The cells of the row each `per` is taken of.
        of: usize,
        /// The fewest cells the share comes to.
        min: usize,
        /// The most cells the share comes to.
        max: usize,
    },
    /// As wide as its own cell on each row, at most this many cells, and
    /// never padded. The columns after it follow it on each row instead
    /// of lining up: a label and the notes after it.
    Hug(usize),
    /// What the other columns leave. The others give way while a fill has
    /// fewer than its minimum, [`MIN_FILL`](super::MIN_FILL) cells unless
    /// told ([`Column::at_least`]). A fill that drops leaves rather than
    /// show fewer, unless its whole text is shorter; one with no rule of
    /// its own narrows last of all. It is cut with `…`.
    Fill,
}

/// Which edge of its column a cell keeps to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// The start: text, chips, marks.
    #[default]
    Left,
    /// The end: counts, numbers.
    Right,
}

/// One column of a [`ColumnSchema`]: the key its cells are given under,
/// its width, the edge its cells keep to, the gap after it, and how it
/// gives way when the row is too narrow.
///
/// A column is built from its width rule ([`fixed`](Self::fixed),
/// [`cap`](Self::cap), [`steps`](Self::steps),
/// [`row_steps`](Self::row_steps), [`share`](Self::share),
/// [`hug`](Self::hug), or [`fill`](Self::fill)) and then told the rest. It
/// keeps its cells unless it [`drops`](Self::drops) or
/// [`shrinks`](Self::shrinks); both take a priority, and the lowest gives
/// way first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column<K> {
    pub(super) key: K,
    pub(super) width: Width,
    pub(super) align: Align,
    pub(super) gap: usize,
    pub(super) shrink: Option<(u8, usize)>,
    pub(super) drop: Option<u8>,
    pub(super) drop_below: usize,
    pub(super) ragged: bool,
    pub(super) absorbs_lead: bool,
    /// The cells a fill wants before the others stop giving way.
    pub(super) fill_min: usize,
}

impl<K> Column<K> {
    const fn of(key: K, width: Width) -> Self {
        Self {
            key,
            width,
            align: Align::Left,
            gap: GAP,
            shrink: None,
            drop: None,
            drop_below: 0,
            ragged: false,
            absorbs_lead: false,
            fill_min: MIN_FILL,
        }
    }

    /// A column `cells` wide on every row ([`Width::Fixed`]).
    pub const fn fixed(key: K, cells: usize) -> Self {
        Self::of(key, Width::Fixed(cells))
    }

    /// A column as wide as its widest cell in the tree, at most `max`
    /// ([`Width::Cap`]).
    pub const fn cap(key: K, max: usize) -> Self {
        Self::of(key, Width::Cap(max))
    }

    /// A column whose width steps with the room left for it
    /// ([`Width::Steps`]).
    pub const fn steps(key: K, steps: &'static [(usize, usize)]) -> Self {
        Self::of(key, Width::Steps(steps))
    }

    /// A column whose width steps with the whole row's, its lead included
    /// ([`Width::RowSteps`]).
    pub const fn row_steps(key: K, steps: &'static [(usize, usize)]) -> Self {
        Self::of(key, Width::RowSteps(steps))
    }

    /// A column that takes `per` of every `of` cells of the whole row, its
    /// lead included, rounded down ([`Width::Share`]). Bound it with
    /// [`within`](Self::within).
    ///
    /// ```
    /// use ratatui::style::Style;
    /// use tuikit::columns::{Cell, Column, ColumnSchema, Measure};
    /// use tuikit::theme::Theme;
    ///
    /// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    /// enum Col {
    ///     Kind,
    ///     Name,
    ///     Note,
    /// }
    ///
    /// // A kind word, a name that grows with the row, and a note that
    /// // takes the rest. The kind gives way first, then the name narrows.
    /// let schema = ColumnSchema::new([
    ///     Column::fixed(Col::Kind, 6).gap(1).drops(0),
    ///     Column::share(Col::Name, 1, 4).within(8, 20).shrinks(1, 4),
    ///     Column::fill(Col::Note).at_least(12),
    /// ]);
    /// let cells = vec![
    ///     (Col::Kind, Cell::text("file", Style::default())),
    ///     (Col::Name, Cell::text("alphabet", Style::default())),
    ///     (Col::Note, Cell::text("the first draft", Style::default())),
    /// ];
    /// let mut measure = Measure::new();
    /// // Laid out after a two-cell cursor gutter, its lead.
    /// measure.add(&schema, 2, &cells);
    /// let wide = schema.layout(&measure, 2, 78);
    /// assert_eq!(wide.width(Col::Name), Some(20), "a quarter of 80, at most 20");
    /// let narrow = schema.layout(&measure, 2, 22);
    /// assert_eq!(narrow.width(Col::Kind), None);
    /// assert_eq!(narrow.width(Col::Name), Some(8));
    /// let row = schema.draw(&measure, 2, 22, &cells, &Theme::default());
    /// assert_eq!(Cell::plain(&row), "alphabet  the first d…");
    /// ```
    pub const fn share(key: K, per: usize, of: usize) -> Self {
        Self::of(
            key,
            Width::Share {
                per,
                of,
                min: 0,
                max: usize::MAX,
            },
        )
    }

    /// Keeps a [`share`](Self::share) column between `min` and `max`
    /// cells. Nothing changes for the other width rules.
    #[must_use]
    pub const fn within(mut self, min: usize, max: usize) -> Self {
        if let Width::Share { per, of, .. } = self.width {
            self.width = Width::Share { per, of, min, max };
        }
        self
    }

    /// A column as wide as its own cell on each row, at most `max`, after
    /// which the row flows ([`Width::Hug`]).
    pub const fn hug(key: K, max: usize) -> Self {
        Self::of(key, Width::Hug(max))
    }

    /// The column that takes what the others leave ([`Width::Fill`]).
    pub const fn fill(key: K) -> Self {
        Self::of(key, Width::Fill)
    }

    /// Makes a fill want `cells` before the others stop giving way, in
    /// place of [`MIN_FILL`]: a snippet that is not worth showing in fewer.
    /// Nothing changes for the other width rules.
    #[must_use]
    pub const fn at_least(mut self, cells: usize) -> Self {
        self.fill_min = cells;
        self
    }

    /// Leaves `cells` blank cells after the column, between it and the
    /// next cell the row shows (the default is [`GAP`]). A gap after the
    /// last cell of a row is not drawn.
    #[must_use]
    pub const fn gap(mut self, cells: usize) -> Self {
        self.gap = cells;
        self
    }

    /// Keeps the column's cells to its end (a count, a number).
    #[must_use]
    pub const fn right(mut self) -> Self {
        self.align = Align::Right;
        self
    }

    /// Makes the column leave the row, its gap with it, when the row is
    /// too narrow, at `priority` (lowest first).
    #[must_use]
    pub const fn drops(mut self, priority: u8) -> Self {
        self.drop = Some(priority);
        self
    }

    /// Makes the column narrow, down to `floor` cells, when the row is too
    /// narrow, at `priority` (lowest first). Its cells are cut by their
    /// own rule. A column that also [`drops`](Self::drops) at a later
    /// priority narrows first and leaves later.
    #[must_use]
    pub const fn shrinks(mut self, priority: u8, floor: usize) -> Self {
        self.shrink = Some((priority, floor));
        self
    }

    /// Leaves the column out of every row whose room (the cells the
    /// schema lays the row out in) is narrower than `room`, whatever else
    /// would fit.
    #[must_use]
    pub const fn drop_below(mut self, room: usize) -> Self {
        self.drop_below = room;
        self
    }

    /// Makes the column ragged: only the cells that something follows on
    /// their row widen it, and a cell that ends its row runs on into the
    /// room left instead of stopping at the column's width, even where the
    /// column gave way for the columns after it on other rows (when the
    /// room holds its floor). For a column that spaces the columns after it
    /// (a description before a count), so a long one on a row with nothing
    /// after it pushes no other row's count away.
    #[must_use]
    pub const fn ragged(mut self) -> Self {
        self.ragged = true;
        self
    }

    /// Makes a capped column end in one place on rows of every lead (a
    /// tree row's indent): it is measured from the start of each row, and
    /// a row's cell gives back the row's lead, so a deeper row's cell is
    /// that much narrower and the columns after it land where a
    /// shallower row's do; its cap bounds each cell, not the column, so a
    /// cell cut to the cap ends there too. The lined-up columns of a row
    /// with a lead then give way as a row with no lead would in the whole
    /// width, so rows at every depth lose the same ones. Without it a column
    /// is measured from the end of the row's lead and moves right with it.
    #[must_use]
    pub const fn absorbs_lead(mut self) -> Self {
        self.absorbs_lead = true;
        self
    }

    /// The key the column's cells are given under.
    pub const fn key(&self) -> K
    where
        K: Copy,
    {
        self.key
    }

    /// The column's width rule.
    pub const fn width(&self) -> Width {
        self.width
    }
}

/// A row's columns, in the order the row draws them.
///
/// The schema says nothing about what a cell shows; a host builds each
/// row's [`Cell`](super::Cell)s under the columns' keys, measures every
/// row of its list once ([`Measure`](super::Measure)), and draws each row
/// from the same schema and measure ([`draw`](Self::draw)), so the rows
/// line up and give way alike. Several schemas may share a key: their
/// columns are one column to the measure, so two kinds of row that should
/// line up (or reserve a mark together) use the same key.
///
/// The columns before the first [`Hug`](Width::Hug) column line up across
/// rows: every row drawn in the same room shows the same ones at the same
/// widths. The columns from the first hugging one on follow each row's
/// own cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnSchema<K> {
    pub(super) columns: Vec<Column<K>>,
    /// Index of the first hugging column: the columns before it line up.
    pub(super) split: usize,
}

impl<K: Copy + Eq> ColumnSchema<K> {
    /// A schema of `columns`, in the order the row draws them.
    pub fn new(columns: impl IntoIterator<Item = Column<K>>) -> Self {
        let columns: Vec<Column<K>> = columns.into_iter().collect();
        let split = columns
            .iter()
            .position(|column| matches!(column.width, Width::Hug(_)))
            .unwrap_or(columns.len());
        Self { columns, split }
    }

    /// The columns, in order.
    pub fn columns(&self) -> &[Column<K>] {
        &self.columns
    }

    /// The column under `key`, if the schema has one.
    pub fn column(&self, key: K) -> Option<&Column<K>> {
        self.columns.iter().find(|column| column.key == key)
    }

    /// Where `key` sits in the schema.
    pub(super) fn index_of(&self, key: K) -> Option<usize> {
        self.columns.iter().position(|column| column.key == key)
    }
}
