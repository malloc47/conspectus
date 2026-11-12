//! Reopening each view where the last run left it: the rows it had
//! expanded and the row it had selected, which the state file names by
//! row key ([`crate::tui_state::row_key`]).
//!
//! The file is read at startup, before any row exists, so each view's
//! saved tree waits until that view first lists rows: the first refresh
//! for the view the TUI opens on, the first switch to each of the others.
//! It is applied then, once, and forgotten, so neither a later refresh nor
//! the operator's own moves are ever undone by it; a regroup before it
//! applies drops it, since its rows belong to the old grouping. Under
//! `--no-resume-view` it is dropped unapplied at the same moment.
//!
//! Applying it: the expanded rows are the saved keys that name a row of
//! the rebuilt tree (a key that names none is ignored); the cursor goes to
//! the saved row, else to the deepest row of the path saved with it that
//! is still listed, else to the first row. Between copies of a row listed
//! twice it takes the one whose rows above it match the saved path best.
//! The groups above the cursor are opened when the saved folds left it
//! hidden, and the pane scrolls to it on the next frame.
//!
//! Writing it: each view's expanded rows are kept only for the rows its
//! tree lists, so a group that is gone leaves the file on the next write;
//! a view the operator left is written as it was when they left it, and
//! a view whose saved tree is still waiting is written back unchanged.

use std::collections::BTreeSet;

use tuikit::tree::ancestors;

use super::*;
use crate::tui_state::{PersistedSelection, PersistedTree, remembers_rows, row_key};

impl App {
    /// Hold the trees the state file saved, by view, until each view
    /// lists rows.
    pub(super) fn hold_saved_trees(
        &mut self,
        views: impl IntoIterator<Item = (View, PersistedTree)>,
    ) {
        for (view, tree) in views {
            if remembers_rows(view) && !tree.is_empty() {
                self.saved_trees.insert(view, tree);
            }
        }
    }

    /// Forget the active view's saved tree without applying it: the
    /// operator asked for the view's top.
    pub(super) fn drop_saved_tree(&mut self) {
        self.saved_trees.remove(&self.active_view);
    }

    /// Apply the active view's saved tree to the rows just built, the
    /// first time the view lists any. Called by every rebuild of the
    /// rows, before the right pane is pointed at the selection.
    pub(super) fn reopen_saved_tree(&mut self) {
        if self.tree.rows.is_empty() {
            return;
        }
        let Some(saved) = self.saved_trees.remove(&self.active_view) else {
            return;
        };
        if !self.config.resume_view {
            return;
        }
        let rows = &self.tree.rows;
        let keys: Vec<Option<String>> = rows.iter().map(|row| row_key(&row.id)).collect();
        if let Some(expanded) = &saved.expanded {
            let ids = rows
                .iter()
                .zip(&keys)
                .filter(|(_, key)| key.as_ref().is_some_and(|key| expanded.contains(key)))
                .map(|(row, _)| row.id.clone())
                .collect();
            self.tree_state.set_expanded(ids);
        }
        match saved.selected.as_ref() {
            Some(selected) => match saved_row(rows, &keys, selected) {
                Some(index) => {
                    self.tree_state.select_at(rows, index);
                }
                None => {
                    self.tree_state.home(rows);
                }
            },
            // New folds can hide the row the first load picked.
            None if self.tree_state.cursor(rows).is_none() => {
                self.tree_state.home(rows);
            }
            None => {}
        }
    }

    /// The active view's tree as the state file keeps it: the expanded
    /// rows its tree lists and the selected row with the rows above it.
    /// Nothing for the Search view or a view with no rows yet.
    pub(super) fn remembered_tree(&self) -> PersistedTree {
        let rows = &self.tree.rows;
        if !remembers_rows(self.active_view) || rows.is_empty() {
            return PersistedTree::default();
        }
        let listed: BTreeSet<&RowId> = rows.iter().map(|row| &row.id).collect();
        let expanded = self
            .tree_state
            .expanded()
            .iter()
            .filter(|id| listed.contains(id))
            .filter_map(row_key)
            .collect();
        let selected = self.tree_state.cursor(rows).and_then(|position| {
            let index = self.tree_state.visible_indices(rows)[position];
            // A row the file cannot name is kept as the nearest row above
            // it that it can, so a restart lands near it.
            let mut named = std::iter::once(index)
                .chain(ancestors(rows, index))
                .filter_map(|index| row_key(&rows[index].id).map(|key| (index, key)));
            let (index, row) = named.next()?;
            let mut path: Vec<String> = ancestors(rows, index)
                .into_iter()
                .filter_map(|above| row_key(&rows[above].id))
                .collect();
            path.reverse();
            Some(PersistedSelection { row, path })
        });
        PersistedTree {
            expanded: Some(expanded),
            selected,
        }
    }

    /// What the state file keeps of `view`'s tree: its saved tree while
    /// that still waits, the active view's tree as it is now, or another
    /// view's as it was when the operator left it.
    pub(super) fn tree_to_persist(
        &self,
        view: View,
        slot: Option<&ViewStateSlot>,
    ) -> PersistedTree {
        if let Some(saved) = self.saved_trees.get(&view) {
            saved.clone()
        } else if view == self.active_view {
            self.remembered_tree()
        } else {
            slot.map(|slot| slot.remembered.clone()).unwrap_or_default()
        }
    }
}

/// The index of the row the cursor goes back to: the saved row, else the
/// deepest row of its saved path still listed, each by the copy whose rows
/// above it best match the path. `None` when none is listed.
fn saved_row(
    rows: &[Row],
    keys: &[Option<String>],
    selected: &PersistedSelection,
) -> Option<usize> {
    std::iter::once((&selected.row, selected.path.as_slice()))
        .chain(
            (0..selected.path.len())
                .rev()
                .map(|depth| (&selected.path[depth], &selected.path[..depth])),
        )
        .find_map(|(key, above)| best_copy(rows, keys, key, above))
}

/// Of the rows named `key`, the one with the most rows above it named in
/// `path`; the first of them on a tie.
fn best_copy(rows: &[Row], keys: &[Option<String>], key: &str, path: &[String]) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for (index, _) in keys
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.as_deref() == Some(key))
    {
        let matched = ancestors(rows, index)
            .into_iter()
            .filter(|&above| {
                keys[above]
                    .as_ref()
                    .is_some_and(|above| path.contains(above))
            })
            .count();
        if best.is_none_or(|(_, most)| matched > most) {
            best = Some((index, matched));
        }
    }
    best.map(|(index, _)| index)
}

#[cfg(test)]
#[path = "restore_tests.rs"]
mod tests;
