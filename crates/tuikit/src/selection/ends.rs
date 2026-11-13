//! What a cursor does at the ends of its list.

use crate::scroll::{clamp_step, wrap_step};

/// What a list's cursor does at the ends of the list (house rule 2).
///
/// A list picks its rule by what it is, not by how long it happens to
/// be: a menu that grew to twenty entries still wraps, and a tree of
/// three rows still stops.
///
/// ```
/// use tuikit::selection::Ends;
///
/// // A menu of four entries: up from the first is the last.
/// assert_eq!(Ends::Wrap.step(0, 4, -1), 3);
/// assert_eq!(Ends::Wrap.step(3, 4, 1), 0);
/// // A tree of forty rows: the cursor stops at either end.
/// assert_eq!(Ends::Clamp.step(0, 40, -1), 0);
/// assert_eq!(Ends::Clamp.step(39, 40, 1), 39);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ends {
    /// Past the last row comes the first, and before the first the
    /// last. For short choice lists, where the far end is a step away:
    /// menus, pickers, checklists, palettes, settings lists, and a
    /// form's ring of fields.
    Wrap,
    /// The cursor stops at the first and the last row. For scrollable
    /// content, where running off the end should not jump back to the
    /// start: trees, property lists, pagers, and logs.
    Clamp,
}

impl Ends {
    /// `index` moved `delta` rows through a list of `len` rows by this
    /// rule: [`wrap_step`] or [`clamp_step`]. Returns 0 for an empty
    /// list and never an index past the end.
    pub fn step(self, index: usize, len: usize, delta: i32) -> usize {
        match self {
            Self::Wrap => wrap_step(index, len, delta),
            Self::Clamp => clamp_step(index, len, delta),
        }
    }
}
