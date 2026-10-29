//! A plan of columns: how a row of several cells shares its width, and
//! which cells give way first when it is narrow.

use crate::text;

/// How wide a column wants to be before any column gives way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Width {
    /// Always this many cells.
    Fixed(usize),
    /// `per` of every `of` cells of the row, rounded down, kept between
    /// `min` and `max`.
    Share {
        per: usize,
        of: usize,
        min: usize,
        max: usize,
    },
    /// The cells of the first step whose row width the row reaches.
    Steps(&'static [(usize, usize)]),
    /// What the other columns leave, wanting at least `min`.
    Fill { min: usize },
}

/// How a column makes room for the fill when the row is short of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GiveWay {
    /// It keeps its cells.
    Never,
    /// It leaves the row, gap and all.
    Drop { order: u8 },
    /// It narrows one cell at a time, down to `to`.
    Shrink { order: u8, to: usize },
}

impl GiveWay {
    fn order(self) -> Option<u8> {
        match self {
            Self::Never => None,
            Self::Drop { order } | Self::Shrink { order, .. } => Some(order),
        }
    }
}

/// Which edge of its cells a column's text keeps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Align {
    Start,
    End,
}

/// One column of a [`ColumnPlan`]: how wide it wants to be, the blank
/// cells after it, how it gives way to the fill, and which edge its text
/// keeps to.
///
/// A column is built from its width rule ([`fixed`](Self::fixed),
/// [`share`](Self::share), [`steps`](Self::steps), or
/// [`fill`](Self::fill)) and then told the rest: its
/// [`gap`](Self::gap), whether it [`drops`](Self::drops) or
/// [`shrinks`](Self::shrinks) when the row is short, and whether its text
/// keeps to the [`end`](Self::end).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    width: Width,
    gap: usize,
    give: GiveWay,
    align: Align,
}

impl Column {
    const fn of(width: Width) -> Self {
        Self {
            width,
            gap: 0,
            give: GiveWay::Never,
            align: Align::Start,
        }
    }

    /// A column `cells` wide whatever the row's width: a word from a
    /// short set, a chip.
    pub const fn fixed(cells: usize) -> Self {
        Self::of(Width::Fixed(cells))
    }

    /// A column that takes `per` of every `of` cells of the row, rounded
    /// down: a label that grows with the row. Bound it with
    /// [`within`](Self::within).
    pub const fn share(per: usize, of: usize) -> Self {
        Self::of(Width::Share {
            per,
            of,
            min: 0,
            max: usize::MAX,
        })
    }

    /// A column whose width steps with the row's: `steps` pairs a row
    /// width with the cells the column takes from that width up, widest
    /// first (`&[(100, 22), (78, 20), (0, 14)]`). A row narrower than
    /// every step gives it nothing.
    pub const fn steps(steps: &'static [(usize, usize)]) -> Self {
        Self::of(Width::Steps(steps))
    }

    /// The column that takes what the others leave: a snippet, a
    /// description. The others give way while it has fewer than `min`
    /// cells. A plan has one fill, its first; a later one gets no cells.
    pub const fn fill(min: usize) -> Self {
        Self::of(Width::Fill { min })
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

    /// Leaves `cells` blank cells after the column while it is shown.
    #[must_use]
    pub const fn gap(mut self, cells: usize) -> Self {
        self.gap = cells;
        self
    }

    /// Makes the column leave the row, its gap with it, when the fill is
    /// short of its minimum. Columns give way in `order`, lowest first
    /// (columns of one order in the plan's order), and stop as soon as the
    /// fill has its minimum.
    #[must_use]
    pub const fn drops(mut self, order: u8) -> Self {
        self.give = GiveWay::Drop { order };
        self
    }

    /// Makes the column narrow one cell at a time, down to `to` cells,
    /// when the fill is short of its minimum, in `order` with the columns
    /// that drop.
    #[must_use]
    pub const fn shrinks(mut self, order: u8, to: usize) -> Self {
        self.give = GiveWay::Shrink { order, to };
        self
    }

    /// Keeps the column's text to the end of its cells (a count, an age)
    /// instead of the start.
    #[must_use]
    pub const fn end(mut self) -> Self {
        self.align = Align::End;
        self
    }

    /// The cells the column wants in a row `width` cells wide, before any
    /// column gives way; a fill wants none yet.
    fn wanted(&self, width: usize) -> usize {
        match self.width {
            Width::Fixed(cells) => cells,
            Width::Share { per, of, min, max } => {
                let share = width.saturating_mul(per).checked_div(of).unwrap_or(0);
                share.max(min).min(max)
            }
            Width::Steps(steps) => steps
                .iter()
                .find(|(from, _)| width >= *from)
                .map_or(0, |(_, cells)| *cells),
            Width::Fill { .. } => 0,
        }
    }
}

/// How the cells of a row share its width: ordered columns, each with a
/// width rule and a gap after it, one fill that takes what the others
/// leave, and the order in which the others give way when the fill is
/// short of its minimum (dropping out, or narrowing to a floor).
///
/// A plan says nothing about what a cell shows; a row lays its plan out
/// for the width it is told ([`layout`](Self::layout)) and draws each
/// cell at the width the [`ColumnLayout`] gives it, so every row drawn
/// from one plan lines up with the others, in a palette or in any list.
///
/// ```
/// use tuikit::palette::{Column, ColumnPlan};
///
/// // A kind word, a name that grows with the row, and a note that takes
/// // the rest. The kind gives way first, then the name narrows.
/// let plan = ColumnPlan::new([
///     Column::fixed(6).gap(1).drops(0),
///     Column::share(1, 4).within(8, 20).gap(2).shrinks(1, 4),
///     Column::fill(12),
/// ]);
/// let wide = plan.layout(60);
/// assert_eq!((wide.width(0), wide.width(1), wide.fill()), (6, 15, 36));
/// let narrow = plan.layout(24);
/// assert_eq!((narrow.width(0), narrow.width(1), narrow.fill()), (0, 8, 14));
/// let tiny = plan.layout(16);
/// assert_eq!((tiny.width(0), tiny.width(1), tiny.fill()), (0, 4, 10));
/// assert_eq!(tiny.fit(1, "alphabet").as_deref(), Some("alp…  "));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColumnPlan {
    lead: usize,
    columns: Vec<Column>,
}

impl ColumnPlan {
    /// A plan of `columns`, in the order the row draws them.
    pub fn new(columns: impl IntoIterator<Item = Column>) -> Self {
        Self {
            lead: 0,
            columns: columns.into_iter().collect(),
        }
    }

    /// Counts `cells` before the first column that the row does not draw
    /// itself (a cursor gutter) in the width the plan shares out, so the
    /// shares and steps of a row drawn after a gutter match those of a
    /// grid that counts it.
    #[must_use]
    pub fn lead(mut self, cells: usize) -> Self {
        self.lead = cells;
        self
    }

    /// How many columns the plan has.
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Whether the plan has no columns.
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// The plan laid out for a row `width` cells wide (the lead included):
    /// every column at the width it wants, then, while the fill is short
    /// of its minimum, the columns that give way, in their order; the
    /// fill takes what is left.
    pub fn layout(&self, width: usize) -> ColumnLayout {
        let fill = self
            .columns
            .iter()
            .position(|column| matches!(column.width, Width::Fill { .. }));
        let wants = fill.map_or(0, |index| match self.columns[index].width {
            Width::Fill { min } => min,
            _ => 0,
        });
        let mut widths: Vec<usize> = self
            .columns
            .iter()
            .map(|column| column.wanted(width))
            .collect();
        let mut givers: Vec<(u8, usize)> = self
            .columns
            .iter()
            .enumerate()
            .filter_map(|(index, column)| column.give.order().map(|order| (order, index)))
            .collect();
        givers.sort_unstable();
        for (_, index) in givers {
            match self.columns[index].give {
                GiveWay::Drop { .. } => {
                    if self.used(&widths, fill) + wants > width {
                        widths[index] = 0;
                    }
                }
                GiveWay::Shrink { to, .. } => {
                    while self.used(&widths, fill) + wants > width && widths[index] > to {
                        widths[index] -= 1;
                    }
                }
                GiveWay::Never => {}
            }
        }
        if let Some(index) = fill {
            widths[index] = width.saturating_sub(self.used(&widths, fill));
        }
        let placed = self
            .columns
            .iter()
            .zip(widths)
            .enumerate()
            .map(|(index, (column, cells))| {
                let shown = cells > 0 || Some(index) == fill;
                Placed {
                    cells,
                    gap: if shown { column.gap } else { 0 },
                    shown,
                    align: column.align,
                }
            })
            .collect();
        ColumnLayout { placed, fill }
    }

    /// The cells the lead and the shown columns other than the fill take
    /// with their gaps (the fill's gap included), at `widths`.
    fn used(&self, widths: &[usize], fill: Option<usize>) -> usize {
        self.columns
            .iter()
            .zip(widths)
            .enumerate()
            .map(|(index, (column, &cells))| {
                if Some(index) == fill {
                    column.gap
                } else if cells > 0 {
                    cells + column.gap
                } else {
                    0
                }
            })
            .sum::<usize>()
            + self.lead
    }
}

/// One column as laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Placed {
    cells: usize,
    gap: usize,
    shown: bool,
    align: Align,
}

/// A [`ColumnPlan`] laid out for one width: the cells each column takes
/// (none for a column that gave way by dropping out), the gap after it,
/// and what is left for the fill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnLayout {
    placed: Vec<Placed>,
    /// Which column is the fill, if the plan has one.
    fill: Option<usize>,
}

impl ColumnLayout {
    /// The cells column `column` takes: 0 when it dropped out, or when
    /// the plan has no such column.
    pub fn width(&self, column: usize) -> usize {
        self.placed.get(column).map_or(0, |placed| placed.cells)
    }

    /// Whether column `column` is in the row. The fill always is, even
    /// with no cells left.
    pub fn is_shown(&self, column: usize) -> bool {
        self.placed.get(column).is_some_and(|placed| placed.shown)
    }

    /// The blank cells after column `column`; 0 when it is not shown.
    pub fn gap(&self, column: usize) -> usize {
        self.placed.get(column).map_or(0, |placed| placed.gap)
    }

    /// The cells left for the fill; 0 for a plan without one.
    pub fn fill(&self) -> usize {
        self.fill.map_or(0, |column| self.width(column))
    }

    /// `text` as column `column` draws it: cut with `…` to the column's
    /// cells, padded to them at the edge it keeps away from, then its
    /// gap. `None` when the column is not shown.
    pub fn fit(&self, column: usize, text: &str) -> Option<String> {
        let placed = self.placed.get(column).filter(|placed| placed.shown)?;
        let cell = match placed.align {
            Align::Start => text::fit(text, placed.cells),
            Align::End => text::pad_start(&text::truncate_end(text, placed.cells), placed.cells),
        };
        Some(format!("{cell}{}", " ".repeat(placed.gap)))
    }
}

#[cfg(test)]
#[path = "columns_tests.rs"]
mod tests;
