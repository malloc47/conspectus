//! Row-tree state: loading data, selection movement, and expansion.
//!
//! The selection, the expanded rows, and the window the tree drew are the
//! framework's [`TreeState`]; this file loads rebuilt rows into it, keeps
//! the selection across the rebuild, and points the right pane at the row
//! a move lands on.

use super::*;

impl App {
    pub(super) fn set_data(
        &mut self,
        snapshot: SnapshotHandle,
        tree: RowTree,
        loaded_at_epoch: i64,
        initial_selection_hint: Option<RowId>,
    ) {
        self.refresh_failure = None;
        self.loaded_at_epoch = Some(loaded_at_epoch);
        let before = self.tree_state.begin_rebuild(&self.tree.rows);
        if before.selected().is_none() {
            self.tree_state.set_expanded(initial_expanded_rows(&tree));
        }
        self.handle = Some(snapshot);
        self.tree = tree;
        self.auto_expand_search_rows();
        // A first load opens on the row the launch context's hint names
        // instead of the leading tree row. Later refreshes keep the
        // operator's selection, so the hint never fights it.
        self.tree_state
            .finish_rebuild(before, &self.tree.rows, initial_selection_hint.as_ref());
        self.recompute_explorer();
    }

    /// Swap the row tree in place after a projection-only rebuild
    /// (ADR 0085 contract 4). Snapshot / staleness / first-load
    /// bookkeeping stay untouched; only the tree and the selection
    /// retention run. `SetTree` never fires before the first
    /// `SetData`, so any tree-less first-load path is out of scope
    /// here — the empty-tree branch is a safety net for corner
    /// cases like an empty snapshot.
    ///
    /// Re-derive the row tree from the held snapshot and current
    /// projection state, then swap it in via [`Self::set_tree`].
    /// Called from the projection-change reducer arms (Msg::SwitchView
    /// / SetGrouping / SetFilter / SetSort). No-op when no snapshot
    /// is loaded yet — the projection change still lands, and the
    /// next Msg::SetData will build the tree against the up-to-date
    /// projection state.
    pub(super) fn rebuild_tree_in_place(&mut self) {
        #[cfg(test)]
        {
            self.tree_rebuilds += 1;
        }
        let Some(db) = self.handle.as_ref() else {
            return;
        };
        let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
            db.snapshot(),
            self,
        ));
        self.set_tree(tree);
        self.auto_expand_search_rows();
    }

    pub(super) fn set_tree(&mut self, tree: RowTree) {
        let before = self.tree_state.begin_rebuild(&self.tree.rows);
        self.tree = tree;
        self.tree_state
            .finish_rebuild(before, &self.tree.rows, None);
        self.recompute_explorer();
    }

    /// Move the selection with one of the tree's moves (a step, a page,
    /// an end), then point the right pane at the row it lands on. With
    /// no rows showing nothing is selected and the right pane is empty.
    pub(super) fn move_selection(
        &mut self,
        step: impl FnOnce(&mut TreeState<RowId>, &[Row]) -> Option<usize>,
    ) {
        self.status_message = None;
        if step(&mut self.tree_state, &self.tree.rows).is_some() {
            self.recompute_explorer();
        } else {
            self.explorer = None;
        }
    }

    /// What the left pane's tree needs to know about `row` that only the
    /// snapshot and the hand-off can say: whether it stands for an entity
    /// that is gone or unknown (a row of the Sessions, Mux, PRs, or Search
    /// view, a stopped mux included, wears the ghost look whole; a Pinned
    /// or History row dims its own cells by its tone, keeping its kind
    /// glyph's color), whether it still shows the values from before the
    /// hand-off (ADR 0108), and the pin marker the Sessions, Mux, PRs, and
    /// Search views put on a pinned entity's row (the Pinned and History
    /// rows carry theirs).
    pub(crate) fn tree_row_state(&self, row: &Row) -> RowState {
        let node = crate::tui::rows::row_node(&row.id);
        let lookup = self.handle.as_ref().zip(node);
        let dims_itself = matches!(&row.kind, RowKind::Mixed(mixed) if mixed.state.is_none());
        let ghost = !dims_itself
            && lookup.is_some_and(|(handle, node)| handle.snapshot().presence(node).is_some());
        let native = matches!(
            self.active_view,
            View::Sessions | View::Mux | View::Prs | View::Search
        );
        let marker = lookup
            .filter(|_| native)
            .and_then(|(handle, node)| handle.pinned_nodes().get(node).copied())
            .unwrap_or(crate::tui::rows::PinMarker::None);
        RowState {
            ghost,
            pending: self.row_is_frozen(row),
            mark: marker.into(),
        }
    }

    /// [`Self::tree_row_state`] for every row of the tree, index for
    /// index. The left pane works these out before each frame, so laying
    /// a row out looks nothing up.
    pub(crate) fn tree_row_states(&self) -> Vec<RowState> {
        self.tree
            .rows
            .iter()
            .map(|row| self.tree_row_state(row))
            .collect()
    }

    #[cfg(test)]
    pub(super) fn visible_rows_owned(&self) -> Vec<RowId> {
        self.visible_rows().iter().map(|r| r.id.clone()).collect()
    }
}

/// Does `row` represent `target` in the left-pane tree?
/// Group rows match when their `primary_node` (when set) equals
/// `target`.
pub(super) fn row_matches(row: &crate::tui::rows::Row, target: &NodeId) -> bool {
    row_primary_node(row) == Some(target)
}

/// The entity `row` stands for in the left-pane tree: a group's node
/// (when it has one), a session, mux, PR, pin, or a pinned or History
/// row's entity. A search hit or pane row stands for none.
pub(super) fn row_primary_node(row: &crate::tui::rows::Row) -> Option<&NodeId> {
    match &row.kind {
        RowKind::Group(group) => group.primary_node.as_ref(),
        RowKind::AgentSession(s) => Some(&s.primary_node),
        RowKind::MuxSession(m) => Some(&m.primary_node),
        RowKind::Pr(p) => Some(&p.primary_node),
        RowKind::Mixed(m) => Some(&m.primary_node),
        RowKind::Hit(_) | RowKind::Pane(_) | RowKind::PaneHit(_) => None,
    }
}

pub(super) fn initial_expanded_rows(tree: &RowTree) -> BTreeSet<RowId> {
    let mut expanded = BTreeSet::new();
    for row in &tree.rows {
        if matches!(row.id, RowId::Bucket(_)) {
            add_expandable_group(&mut expanded, row);
        }
    }
    let mut launch_indices: Vec<usize> = tree
        .rows
        .iter()
        .enumerate()
        .filter_map(|(idx, row)| match &row.kind {
            RowKind::Group(group) if group.is_launch_context => Some(idx),
            _ => None,
        })
        .collect();
    if launch_indices.is_empty()
        && let Some((idx, _)) = tree
            .rows
            .iter()
            .enumerate()
            .find(|(_, row)| matches!(row.kind, RowKind::Group(_)))
    {
        launch_indices.push(idx);
    }

    for idx in launch_indices {
        let launch_depth = tree.rows[idx].depth;
        add_expandable_group(&mut expanded, &tree.rows[idx]);

        let mut next_ancestor_depth = launch_depth;
        for ancestor in tree.rows[..idx].iter().rev() {
            if ancestor.depth < next_ancestor_depth {
                add_expandable_group(&mut expanded, ancestor);
                next_ancestor_depth = ancestor.depth;
            }
        }

        for descendant in tree.rows[idx + 1..]
            .iter()
            .take_while(|row| row.depth > launch_depth)
        {
            add_expandable_group(&mut expanded, descendant);
        }
    }

    expanded
}

pub(super) fn add_expandable_group(expanded: &mut BTreeSet<RowId>, row: &Row) {
    if row.expandable && matches!(row.kind, RowKind::Group(_)) {
        expanded.insert(row.id.clone());
    }
}
