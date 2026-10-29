//! The rows a tree is made of, and which of them show.

use std::collections::BTreeSet;

/// A row of a tree: an identity, a depth, and whether it has children.
///
/// A tree is a flat list of rows in display order. A row's children are
/// the deeper rows that follow it, up to the next row at its depth or
/// shallower, so the list needs no parent links. The identity is what a
/// cursor and an expanded row are remembered by, so it should name the
/// same thing every time the host rebuilds its rows; the same identity
/// may appear more than once (one item listed under two parents).
///
/// A host implements the trait on its own row type, or uses [`Row`].
pub trait TreeRow {
    /// The row's identity, stable across rebuilds.
    type Id: Ord + Clone;

    /// The row's identity.
    fn id(&self) -> &Self::Id;

    /// How deep the row sits: 0 at the top level, one more per level.
    fn depth(&self) -> u16;

    /// Whether the row has children it can show and hide. Only such a
    /// row expands, collapses, or hides the rows below it.
    fn expandable(&self) -> bool;
}

/// The plain tree row: an identity, a depth, whether it has children,
/// and the host's `payload`, which the tree never looks inside.
///
/// ```
/// use tuikit::tree::{Row, TreeRow};
///
/// let row = Row { id: 7, depth: 1, expandable: false, payload: "leaf" };
/// assert_eq!(*row.id(), 7);
/// assert_eq!(row.depth(), 1);
/// assert!(!TreeRow::expandable(&row));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Row<Id, R> {
    /// The row's identity, stable across rebuilds.
    pub id: Id,
    /// How deep the row sits: 0 at the top level.
    pub depth: u16,
    /// Whether the row has children it can show and hide.
    pub expandable: bool,
    /// What the host draws the row from.
    pub payload: R,
}

impl<Id: Ord + Clone, R> TreeRow for Row<Id, R> {
    type Id = Id;

    fn id(&self) -> &Id {
        &self.id
    }

    fn depth(&self) -> u16 {
        self.depth
    }

    fn expandable(&self) -> bool {
        self.expandable
    }
}

/// The indices of the `rows` that show, in display order: a row is hidden
/// while one of its ancestors is an expandable row that is not in
/// `expanded`.
pub(crate) fn visible_indices<R: TreeRow>(rows: &[R], expanded: &BTreeSet<R::Id>) -> Vec<usize> {
    let mut shown = Vec::with_capacity(rows.len());
    // The depth of the nearest collapsed ancestor: every deeper row below
    // it is hidden, and the first row at its depth or shallower ends it.
    let mut hidden_below: Option<u16> = None;
    for (index, row) in rows.iter().enumerate() {
        if hidden_below.is_some_and(|depth| row.depth() > depth) {
            continue;
        }
        hidden_below = None;
        shown.push(index);
        if row.expandable() && !expanded.contains(row.id()) {
            hidden_below = Some(row.depth());
        }
    }
    shown
}

/// The indices of the rows above `index` that hold it: each shallower row
/// met walking up from it, nearest first.
pub(crate) fn ancestors<R: TreeRow>(rows: &[R], index: usize) -> Vec<usize> {
    let Some(row) = rows.get(index) else {
        return Vec::new();
    };
    let mut wanted = row.depth();
    let mut found = Vec::new();
    for (above, candidate) in rows[..index].iter().enumerate().rev() {
        if wanted == 0 {
            break;
        }
        if candidate.depth() < wanted {
            wanted = candidate.depth();
            found.push(above);
        }
    }
    found
}
