//! What the user did to a tree: the cursor, the folds, and the window.

use std::collections::BTreeSet;

use super::row::{TreeRow, ancestors, visible_indices};
use crate::scroll::ScrollWindow;
use crate::selection::Ends;

/// A tree's cursor, folds, and window, kept apart from its rows.
///
/// The state holds no rows. The host owns them and rebuilds them whenever
/// its data changes, so every method that needs them takes them, and the
/// state remembers rows only by their identity ([`TreeRow::Id`]):
///
/// - **The cursor** is the selected row's identity and the position among
///   the showing rows it was last placed at. The position tells copies of
///   an identity listed more than once apart: the cursor is on the copy
///   nearest it ([`cursor`](Self::cursor)).
/// - **The folds** are the set of expanded rows. A row shows while none of
///   its ancestors is a collapsed expandable row
///   ([`visible`](Self::visible)).
/// - **The window** is the one the last frame drew
///   ([`window`](Self::window)): the next frame starts from it, and page
///   keys move the cursor by the rows it showed. A draw returns the window
///   it drew and the host keeps it with [`set_viewport`](Self::set_viewport);
///   nothing is sent from inside the draw.
///
/// Moving the cursor stops at the first and the last showing row (a tree
/// is scrollable content, house rule 2). Rebuilding the rows keeps the
/// cursor on the same identity when it still shows, and otherwise near
/// where it was ([`begin_rebuild`](Self::begin_rebuild)).
///
/// A host that switches between trees (one per view) keeps one state per
/// tree; the state is a plain value it can store and restore.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeState<Id> {
    /// The selected row's identity.
    selected: Option<Id>,
    /// The position among the showing rows the cursor was last placed at,
    /// which picks between copies of an identity. `None` after a jump to
    /// an identity, which lands on its first showing copy.
    position: Option<usize>,
    /// The rows whose children show.
    expanded: BTreeSet<Id>,
    /// The window the last frame drew.
    window: ScrollWindow,
}

impl<Id> Default for TreeState<Id> {
    fn default() -> Self {
        Self {
            selected: None,
            position: None,
            expanded: BTreeSet::new(),
            window: ScrollWindow::default(),
        }
    }
}

/// Where the cursor stood before the rows were rebuilt, as
/// [`TreeState::begin_rebuild`] took it off them: the identity it was on
/// and the position that row showed at. [`TreeState::finish_rebuild`]
/// puts it back.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "a rebuild that is begun is finished with `finish_rebuild`"]
pub struct Rebuild<Id> {
    selected: Option<Id>,
    position: Option<usize>,
}

impl<Id> Rebuild<Id> {
    /// The identity the cursor was on, if any.
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// The position among the showing rows the cursor's row showed at, if
    /// it showed.
    pub const fn position(&self) -> Option<usize> {
        self.position
    }
}

/// The position of the copy of `id` nearest `near` among `ids`, the first
/// copy when `near` is `None` or two copies are as near. `None` when no
/// copy is listed.
fn nearest<'a, Id: PartialEq + 'a>(
    ids: impl IntoIterator<Item = &'a Id>,
    id: &Id,
    near: Option<usize>,
) -> Option<usize> {
    let mut copies = ids
        .into_iter()
        .enumerate()
        .filter_map(|(position, candidate)| (candidate == id).then_some(position));
    let first = copies.next()?;
    let Some(near) = near else {
        return Some(first);
    };
    let mut best = first;
    for position in copies {
        if position.abs_diff(near) < best.abs_diff(near) {
            best = position;
        }
    }
    Some(best)
}

impl<Id: Ord + Clone> TreeState<Id> {
    /// A state with nothing selected, nothing expanded, and an empty
    /// window.
    pub fn new() -> Self {
        Self::default()
    }

    // ---- the cursor ---------------------------------------------------

    /// The selected row's identity, `None` when nothing is selected.
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// The position among the showing rows the cursor was last placed at,
    /// `None` before the first move and after a jump to an identity.
    pub const fn position(&self) -> Option<usize> {
        self.position
    }

    /// Put the cursor on `id` when one of `rows` has it, shown or not (a
    /// host [`reveal`](Self::reveal)s a hidden row first). A jump has no
    /// position to keep, so between copies of `id` the cursor is on the
    /// first that shows. Returns whether a row has `id`; when none does,
    /// nothing changes.
    pub fn select<R: TreeRow<Id = Id>>(&mut self, rows: &[R], id: Id) -> bool {
        if !rows.iter().any(|row| row.id() == &id) {
            return false;
        }
        self.selected = Some(id);
        self.position = None;
        true
    }

    /// Select nothing.
    pub fn deselect(&mut self) {
        self.selected = None;
        self.position = None;
    }

    /// The position of the cursor among the showing `rows`: the copy of
    /// the selected identity nearest the kept position, or the first copy
    /// when no position is kept. `None` when nothing is selected or the
    /// selected row does not show.
    pub fn cursor<R: TreeRow<Id = Id>>(&self, rows: &[R]) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        let shown = self.visible_indices(rows);
        nearest(
            shown.iter().map(|&index| rows[index].id()),
            selected,
            self.position,
        )
    }

    /// Move the cursor `delta` showing rows, down when positive, stopping
    /// at the first and the last. From no selection, or from a row that no
    /// longer shows, it moves from the first row. Returns the position it
    /// lands on, or `None` (and selects nothing) when no row shows.
    pub fn step<R: TreeRow<Id = Id>>(&mut self, rows: &[R], delta: i32) -> Option<usize> {
        let shown = self.visible_indices(rows);
        let from = self.cursor_among(rows, &shown).unwrap_or(0);
        self.place(rows, &shown, Ends::Clamp.step(from, shown.len(), delta))
    }

    /// Move the cursor down one page: the rows the last frame showed
    /// ([`ScrollWindow::page`]), or one row before the first frame.
    /// Returns what [`step`](Self::step) does.
    pub fn page_down<R: TreeRow<Id = Id>>(&mut self, rows: &[R]) -> Option<usize> {
        self.step(rows, self.page())
    }

    /// Move the cursor up one page, as [`page_down`](Self::page_down) moves
    /// it down.
    pub fn page_up<R: TreeRow<Id = Id>>(&mut self, rows: &[R]) -> Option<usize> {
        self.step(rows, -self.page())
    }

    /// Put the cursor on the first showing row. Returns its position (0),
    /// or `None` when no row shows.
    pub fn home<R: TreeRow<Id = Id>>(&mut self, rows: &[R]) -> Option<usize> {
        let shown = self.visible_indices(rows);
        self.place(rows, &shown, 0)
    }

    /// Put the cursor on the last showing row. Returns its position, or
    /// `None` when no row shows.
    pub fn end<R: TreeRow<Id = Id>>(&mut self, rows: &[R]) -> Option<usize> {
        let shown = self.visible_indices(rows);
        self.place(rows, &shown, shown.len().saturating_sub(1))
    }

    /// The rows a page key moves: the window's page, which fits an `i32`
    /// for any terminal.
    fn page(&self) -> i32 {
        i32::try_from(self.window.page()).unwrap_or(i32::MAX)
    }

    /// The cursor's position among `shown`, the showing indices of `rows`.
    pub(crate) fn cursor_among<R: TreeRow<Id = Id>>(
        &self,
        rows: &[R],
        shown: &[usize],
    ) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        nearest(
            shown.iter().map(|&index| rows[index].id()),
            selected,
            self.position,
        )
    }

    /// Select the showing row at `position` (clamped to the last), or
    /// nothing when no row shows.
    fn place<R: TreeRow<Id = Id>>(
        &mut self,
        rows: &[R],
        shown: &[usize],
        position: usize,
    ) -> Option<usize> {
        let Some(last) = shown.len().checked_sub(1) else {
            self.deselect();
            return None;
        };
        let position = position.min(last);
        self.selected = Some(rows[shown[position]].id().clone());
        self.position = Some(position);
        Some(position)
    }

    // ---- the folds ----------------------------------------------------

    /// Whether the row `id` is expanded.
    pub fn is_expanded(&self, id: &Id) -> bool {
        self.expanded.contains(id)
    }

    /// The expanded rows' identities.
    pub const fn expanded(&self) -> &BTreeSet<Id> {
        &self.expanded
    }

    /// Replace the expanded rows, as a host does when it opens a tree on
    /// its default folds.
    pub fn set_expanded(&mut self, expanded: BTreeSet<Id>) {
        self.expanded = expanded;
    }

    /// Expand the row `id`, whatever its rows say. Returns whether it was
    /// collapsed.
    pub fn expand_id(&mut self, id: Id) -> bool {
        self.expanded.insert(id)
    }

    /// Whether the selected row can expand: some row of `rows` with its
    /// identity is expandable.
    fn selected_expands<R: TreeRow<Id = Id>>(&self, rows: &[R]) -> Option<Id> {
        let selected = self.selected.as_ref()?;
        rows.iter()
            .any(|row| row.id() == selected && row.expandable())
            .then(|| selected.clone())
    }

    /// Expand the selected row so its children show. Does nothing on a row
    /// without children or one already expanded. Returns whether the folds
    /// changed.
    pub fn expand<R: TreeRow<Id = Id>>(&mut self, rows: &[R]) -> bool {
        self.selected_expands(rows)
            .is_some_and(|id| self.expanded.insert(id))
    }

    /// Collapse the selected row so its children hide. The cursor stays
    /// on it. Does nothing on a row without children or one already
    /// collapsed. Returns whether the folds changed.
    pub fn collapse<R: TreeRow<Id = Id>>(&mut self, rows: &[R]) -> bool {
        self.selected_expands(rows)
            .is_some_and(|id| self.expanded.remove(&id))
    }

    /// Expand the selected row if it is collapsed, collapse it if it is
    /// expanded. Does nothing on a row without children. Returns whether
    /// the folds changed.
    pub fn toggle<R: TreeRow<Id = Id>>(&mut self, rows: &[R]) -> bool {
        let Some(id) = self.selected_expands(rows) else {
            return false;
        };
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        true
    }

    /// Expand every collapsed ancestor of the row at `index` in `rows`, so
    /// it shows. Rows without children are left as they are.
    pub fn reveal<R: TreeRow<Id = Id>>(&mut self, rows: &[R], index: usize) {
        for above in ancestors(rows, index) {
            let row = &rows[above];
            if row.expandable() {
                self.expanded.insert(row.id().clone());
            }
        }
    }

    // ---- which rows show ----------------------------------------------

    /// The `rows` that show, in display order: a row is hidden while one
    /// of its ancestors is collapsed.
    pub fn visible<'r, R: TreeRow<Id = Id>>(&self, rows: &'r [R]) -> Vec<&'r R> {
        self.visible_indices(rows)
            .into_iter()
            .map(|index| &rows[index])
            .collect()
    }

    /// The indices into `rows` of the rows that show, in display order.
    pub fn visible_indices<R: TreeRow<Id = Id>>(&self, rows: &[R]) -> Vec<usize> {
        visible_indices(rows, &self.expanded)
    }

    // ---- rebuilding ---------------------------------------------------

    /// Take the cursor off `rows`, which the host is about to rebuild,
    /// remembering the identity it was on and the position its row showed
    /// at. Until [`finish_rebuild`](Self::finish_rebuild) nothing is
    /// selected; the host may change the folds in between (open the
    /// rebuilt tree's new rows, say).
    pub fn begin_rebuild<R: TreeRow<Id = Id>>(&mut self, rows: &[R]) -> Rebuild<Id> {
        let position = self.cursor(rows);
        let selected = self.selected.take();
        self.position = None;
        Rebuild { selected, position }
    }

    /// Put the cursor back on the rebuilt `rows`, by the first rule that
    /// applies:
    ///
    /// 1. on the identity it was on, when a row with it shows (the copy
    ///    nearest the position it showed at);
    /// 2. at the position it showed at, or the last showing row when fewer
    ///    show, when that identity is gone;
    /// 3. on `hint`, when nothing was selected before and a row with it
    ///    shows (a host's first load opens on a row it picks);
    /// 4. on the first showing row.
    ///
    /// When no row shows, nothing is selected.
    pub fn finish_rebuild<R: TreeRow<Id = Id>>(
        &mut self,
        before: Rebuild<Id>,
        rows: &[R],
        hint: Option<&Id>,
    ) {
        let shown = self.visible_indices(rows);
        if shown.is_empty() {
            self.deselect();
            return;
        }
        let ids = || shown.iter().map(|&index| rows[index].id());
        let position = before
            .selected
            .as_ref()
            .and_then(|id| nearest(ids(), id, before.position))
            .or(before.position)
            .or_else(|| {
                before
                    .selected
                    .is_none()
                    .then(|| hint.and_then(|hint| nearest(ids(), hint, None)))
                    .flatten()
            })
            .unwrap_or(0);
        self.place(rows, &shown, position);
    }

    /// Rebuild in one step, for a host that changes nothing between:
    /// [`begin_rebuild`](Self::begin_rebuild) over the old rows, then
    /// [`finish_rebuild`](Self::finish_rebuild) over the new.
    pub fn rebuild<R: TreeRow<Id = Id>>(&mut self, old: &[R], new: &[R], hint: Option<&Id>) {
        let before = self.begin_rebuild(old);
        self.finish_rebuild(before, new, hint);
    }

    // ---- the window ---------------------------------------------------

    /// The window the last frame drew: its offset, the rows it showed, and
    /// the showing rows there were. Empty before the first frame.
    pub const fn window(&self) -> ScrollWindow {
        self.window
    }

    /// Keep the window a frame drew, which [`TreeView::render`] returns:
    /// the next frame starts from it and page keys move by its height.
    ///
    /// [`TreeView::render`]: super::TreeView::render
    pub fn set_viewport(&mut self, window: ScrollWindow) {
        self.window = window;
    }

    /// Scroll the kept window to the top, as a host does when it opens the
    /// tree afresh.
    pub fn scroll_to_top(&mut self) {
        self.window.to_top();
    }

    /// The window a frame of `viewport` rows shows over `rows`: the kept
    /// window measured again over the rows that show, then moved as little
    /// as it must to show the cursor's row.
    pub fn placed_window<R: TreeRow<Id = Id>>(&self, rows: &[R], viewport: usize) -> ScrollWindow {
        let shown = self.visible_indices(rows);
        self.window_over(rows, &shown, viewport)
    }

    /// [`placed_window`](Self::placed_window) over the showing indices
    /// `shown`.
    pub(crate) fn window_over<R: TreeRow<Id = Id>>(
        &self,
        rows: &[R],
        shown: &[usize],
        viewport: usize,
    ) -> ScrollWindow {
        let mut window = self.window;
        window.resize(viewport, shown.len());
        if let Some(cursor) = self.cursor_among(rows, shown) {
            window.follow(cursor..cursor + 1);
        }
        window
    }
}
