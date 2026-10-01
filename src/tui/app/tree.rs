//! Row-tree state: loading data, selection movement, and expansion.

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
        let prev_selection = self.selection.take();
        let is_first_load = prev_selection.is_none();
        let prev_visible_index = prev_selection
            .as_ref()
            .and_then(|id| self.visible_rows().iter().position(|r| &r.id == id));
        if is_first_load {
            self.expanded = initial_expanded_rows(&tree);
        }
        self.handle = Some(snapshot);
        self.tree = tree;

        let visible = self.visible_rows_owned();
        if visible.is_empty() {
            self.selection = None;
            self.last_visible_index = None;
        } else if let Some(prev) = prev_selection.as_ref()
            && let Some(pos) = self.position_closest_to(&visible, prev, prev_visible_index)
        {
            self.selection = Some(visible[pos].clone());
            self.last_visible_index = Some(pos);
        } else if let Some(prev_index) = prev_visible_index {
            let clamped = prev_index.min(visible.len() - 1);
            self.selection = Some(visible[clamped].clone());
            self.last_visible_index = Some(clamped);
        } else if is_first_load
            && let Some(hint) = initial_selection_hint
            && let Some(pos) = visible.iter().position(|id| id == &hint)
        {
            // First-load launch-context hint: pre-select the row
            // the operator's cwd points at instead of the leading
            // tree row. Only honored on the first SetData so later
            // refreshes don't fight the operator's manual
            // selection.
            self.selection = Some(hint);
            self.last_visible_index = Some(pos);
        } else {
            self.selection = Some(visible[0].clone());
            self.last_visible_index = Some(0);
        }
        self.recompute_detail();
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
        let Some(db) = self.handle.as_ref() else {
            return;
        };
        let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
            db.snapshot(),
            self,
        ));
        self.set_tree(tree);
    }

    pub(super) fn set_tree(&mut self, tree: RowTree) {
        let prev_selection = self.selection.take();
        let prev_visible_index = prev_selection
            .as_ref()
            .and_then(|id| self.visible_rows().iter().position(|r| &r.id == id));
        self.tree = tree;

        let visible = self.visible_rows_owned();
        if visible.is_empty() {
            self.selection = None;
            self.last_visible_index = None;
        } else if let Some(prev) = prev_selection.as_ref()
            && let Some(pos) = self.position_closest_to(&visible, prev, prev_visible_index)
        {
            self.selection = Some(visible[pos].clone());
            self.last_visible_index = Some(pos);
        } else if let Some(prev_index) = prev_visible_index {
            let clamped = prev_index.min(visible.len() - 1);
            self.selection = Some(visible[clamped].clone());
            self.last_visible_index = Some(clamped);
        } else {
            self.selection = Some(visible[0].clone());
            self.last_visible_index = Some(0);
        }
        self.recompute_detail();
    }

    /// Find `selection` in `visible`, preferring the occurrence
    /// closest to `hint` when the id appears more than once.
    /// Returns `None` when the id is gone (the refresh dropped the
    /// row). Used by [`Self::set_data`] so refreshes don't snap a
    /// stable cursor to the first copy of a duplicated row.
    pub(super) fn position_closest_to(
        &self,
        visible: &[RowId],
        selection: &RowId,
        hint: Option<usize>,
    ) -> Option<usize> {
        let mut matches = visible
            .iter()
            .enumerate()
            .filter_map(|(idx, id)| (id == selection).then_some(idx));
        let first = matches.next()?;
        let Some(hint) = hint else {
            return Some(first);
        };
        let mut best = first;
        let mut best_distance = first.abs_diff(hint);
        for idx in matches {
            let distance = idx.abs_diff(hint);
            if distance < best_distance {
                best = idx;
                best_distance = distance;
            }
        }
        Some(best)
    }

    pub(super) fn move_selection(&mut self, delta: i32) {
        self.status_message = None;
        self.detail_links_expanded = false;
        let visible = self.visible_rows_owned();
        if visible.is_empty() {
            self.selection = None;
            self.detail = None;
            self.last_visible_index = None;
            return;
        }
        let current = self
            .selection
            .as_ref()
            .map_or(0, |id| self.current_visible_index(&visible, id));
        let len = visible.len() as i32;
        let target = (current as i32 + delta).clamp(0, len - 1) as usize;
        self.selection = Some(visible[target].clone());
        self.last_visible_index = Some(target);
        self.recompute_detail();
    }

    pub(super) fn move_selection_to(&mut self, index: usize) {
        self.status_message = None;
        self.detail_links_expanded = false;
        let visible = self.visible_rows_owned();
        if visible.is_empty() {
            self.selection = None;
            self.detail = None;
            self.last_visible_index = None;
            return;
        }
        let clamped = index.min(visible.len() - 1);
        self.selection = Some(visible[clamped].clone());
        self.last_visible_index = Some(clamped);
        self.recompute_detail();
    }

    /// Resolve the visible-row index of the currently-selected
    /// `RowId`. When the id appears more than once (mux view: same
    /// session under multiple candidate muxes), return the
    /// occurrence closest to `last_visible_index` so navigation
    /// reads as "step away from where I am", not "step away from
    /// the first copy in the tree". Falls back to the first
    /// occurrence (or zero if the id is gone) when no cached index
    /// exists.
    pub(super) fn current_visible_index(&self, visible: &[RowId], selection: &RowId) -> usize {
        let mut matches = visible
            .iter()
            .enumerate()
            .filter_map(|(idx, id)| (id == selection).then_some(idx));
        let Some(first) = matches.next() else {
            return 0;
        };
        let Some(cached) = self.last_visible_index else {
            return first;
        };
        let mut best = first;
        let mut best_distance = first.abs_diff(cached);
        for idx in matches {
            let distance = idx.abs_diff(cached);
            if distance < best_distance {
                best = idx;
                best_distance = distance;
            }
        }
        best
    }

    pub(super) fn toggle_expand_selected(&mut self) {
        let Some(id) = self.selection.clone() else {
            return;
        };
        let is_expandable = self.tree.rows.iter().any(|r| r.id == id && r.expandable);
        if !is_expandable {
            return;
        }
        if self.expanded.contains(&id) {
            self.expanded.remove(&id);
        } else {
            self.expanded.insert(id);
        }
    }

    pub(super) fn expand_selected(&mut self) {
        let Some(id) = self.selection.clone() else {
            return;
        };
        let is_expandable = self.tree.rows.iter().any(|r| r.id == id && r.expandable);
        if !is_expandable {
            return;
        }
        self.expanded.insert(id);
    }

    pub(super) fn collapse_selected(&mut self) {
        let Some(id) = self.selection.clone() else {
            return;
        };
        let is_expandable = self.tree.rows.iter().any(|r| r.id == id && r.expandable);
        if !is_expandable {
            return;
        }
        self.expanded.remove(&id);
    }

    pub(super) fn visible_rows_owned(&self) -> Vec<RowId> {
        self.visible_rows().iter().map(|r| r.id.clone()).collect()
    }

    pub(super) fn toggle_linked_details(&mut self) {
        let has_linked_details = self.detail.as_ref().is_some_and(|detail| {
            detail
                .header_fields
                .iter()
                .any(|field| !field.expanded_fields.is_empty())
        });
        if has_linked_details {
            self.detail_links_expanded = !self.detail_links_expanded;
            self.status_message = None;
        } else {
            self.status_message = Some("detail: no linked entities to expand".to_string());
        }
    }
}

/// Does `row` represent `target` in the left-pane tree?
/// Group rows match when their `primary_node` (when set) equals
/// `target`; mux candidate rows match their parent mux's node id.
pub(super) fn row_matches(row: &crate::tui::rows::Row, target: &NodeId) -> bool {
    match &row.kind {
        RowKind::Group(group) => group.primary_node.as_ref() == Some(target),
        RowKind::AgentSession(s) => &s.primary_node == target,
        RowKind::AgentSessionMuxCandidate(c) => &c.primary_node == target,
        RowKind::MuxSession(m) => &m.primary_node == target,
        RowKind::Pr(p) => &p.primary_node == target,
        RowKind::Fork(f) => &f.primary_node == target,
        // Unbound pin rows have no underlying graph node — they
        // never match a follow-sync `target`.
        RowKind::Pin(_) => false,
        RowKind::Repo(r) => &r.primary_node == target,
    }
}

pub(super) fn row_pin_id(row: &crate::tui::rows::Row) -> Option<&str> {
    match &row.kind {
        RowKind::AgentSession(session) => session.pin_id.as_deref(),
        RowKind::MuxSession(mux) => mux.pin_id.as_deref(),
        RowKind::Pin(pin) => Some(pin.pin_id.as_str()),
        _ => None,
    }
}

pub(super) fn initial_expanded_rows(tree: &RowTree) -> BTreeSet<RowId> {
    let mut expanded = BTreeSet::new();
    for row in &tree.rows {
        if matches!(row.id, RowId::Synthetic("pins")) {
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
