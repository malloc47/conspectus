//! The measure pass: how wide each column's cells are over a whole list.

use super::cell::Cell;
use super::schema::{ColumnSchema, Width};

/// What the measure pass found under one key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Found {
    /// The widest cell.
    widest: usize,
    /// The widest cell with the lead of its row (its indent) added.
    widest_with_lead: usize,
    /// Some row has a cell that is not blank.
    any: bool,
}

/// The widths of a list's cells, measured once over every row it has
/// (rows a fold hides included, so folding never moves a column), keyed
/// by column key.
///
/// [`add`](Self::add) measures one row: the cells it gives under its
/// schema's keys, and any other keyed cells it gives, which a host that
/// still draws part of a row itself reads back with
/// [`widest`](Self::widest). A [`ragged`](super::Column::ragged) column
/// is widened only by the cells that something follows on their row, and
/// a capped column ([`Width::Cap`](super::Width::Cap)) counts each cell
/// at most its cap. The pass also keeps the least lead a row had
/// ([`least_lead`](Self::least_lead)): the lead every row of the list
/// shares, such as a cursor gutter.
///
/// ```
/// use ratatui::style::Style;
/// use tuikit::columns::{Cell, Column, ColumnSchema, Measure};
///
/// let schema = ColumnSchema::new([Column::cap("name", 12), Column::fixed("size", 4)]);
/// let mut measure = Measure::new();
/// for name in ["alpha", "a much longer name"] {
///     measure.add(&schema, 0, &[("name", Cell::text(name, Style::default()))]);
/// }
/// // The longer name counts as the column's cap.
/// assert_eq!(measure.widest("name"), 12);
/// assert!(measure.any("name"));
/// assert!(!measure.any("size"));
/// assert_eq!(measure.least_lead(), 0);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measure<K> {
    found: Vec<(K, Found)>,
    /// The least lead of a row measured; `None` before the first.
    least_lead: Option<usize>,
}

impl<K> Default for Measure<K> {
    fn default() -> Self {
        Self {
            found: Vec::new(),
            least_lead: None,
        }
    }
}

impl<K: Copy + Eq> Measure<K> {
    /// Nothing measured yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Measures one row drawn from `schema` after `lead` cells of its own
    /// (a tree row's indent), whose cells are `cells`.
    pub fn add(&mut self, schema: &ColumnSchema<K>, lead: usize, cells: &[(K, Cell)]) {
        self.least_lead = Some(self.least_lead.map_or(lead, |least| least.min(lead)));
        let filled: Vec<bool> = schema
            .columns
            .iter()
            .map(|column| lookup(cells, column.key).is_some_and(|cell| !cell.is_empty()))
            .collect();
        for (index, column) in schema.columns.iter().enumerate() {
            let Some(cell) = lookup(cells, column.key).filter(|cell| !cell.is_empty()) else {
                continue;
            };
            let followed = filled[index + 1..].iter().any(|filled| *filled);
            // A capped column counts each cell at most its cap, so a
            // column that absorbs the lead ends where its capped cells
            // do, at every depth.
            let cap = match column.width {
                Width::Cap(max) => max,
                _ => usize::MAX,
            };
            let width = if column.ragged && !followed {
                None
            } else {
                Some(cell.width().min(cap))
            };
            self.record(column.key, lead, width);
        }
        for (key, cell) in cells {
            if schema.index_of(*key).is_none() && !cell.is_empty() {
                self.record(*key, lead, Some(cell.width()));
            }
        }
    }

    fn record(&mut self, key: K, lead: usize, width: Option<usize>) {
        let index = self
            .found
            .iter()
            .position(|(have, _)| *have == key)
            .unwrap_or_else(|| {
                self.found.push((key, Found::default()));
                self.found.len() - 1
            });
        let found = &mut self.found[index].1;
        found.any = true;
        if let Some(width) = width {
            found.widest = found.widest.max(width);
            found.widest_with_lead = found.widest_with_lead.max(lead + width);
        }
    }

    fn found(&self, key: K) -> Found {
        self.found
            .iter()
            .find(|(have, _)| *have == key)
            .map(|(_, found)| *found)
            .unwrap_or_default()
    }

    /// The widest cell measured under `key`; 0 when none was.
    pub fn widest(&self, key: K) -> usize {
        self.found(key).widest
    }

    /// The widest cell measured under `key` with its row's lead added: how
    /// far from the start of a row the column's widest cell reaches, for a
    /// column that should end in one place at every depth.
    pub fn widest_with_lead(&self, key: K) -> usize {
        self.found(key).widest_with_lead
    }

    /// Whether any row has a cell under `key` that is not blank.
    pub fn any(&self, key: K) -> bool {
        self.found(key).any
    }

    /// The least lead a measured row had (0 before any row is measured):
    /// the lead every row of the list shares, such as a list's cursor
    /// gutter, beside each row's own (a tree row's indent). A column that
    /// [absorbs the lead](super::Column::absorbs_lead) absorbs only a
    /// row's own.
    pub fn least_lead(&self) -> usize {
        self.least_lead.unwrap_or(0)
    }
}

/// The cell `cells` holds under `key`.
pub(super) fn lookup<K: Copy + Eq>(cells: &[(K, Cell)], key: K) -> Option<&Cell> {
    cells
        .iter()
        .find(|(have, _)| *have == key)
        .map(|(_, cell)| cell)
}
