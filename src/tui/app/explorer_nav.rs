//! Right-pane explorer state: detail recompute, cursor, drill, and back.

use super::tree::row_matches;
use super::*;

impl App {
    pub(super) fn recompute_detail(&mut self) {
        self.detail = None;
        self.preview_scroll = 0;
        let Some(selection) = self.selection.as_ref() else {
            self.explorer = None;
            return;
        };
        let Some(handle) = self.handle.as_ref() else {
            self.explorer = None;
            return;
        };
        let snapshot = handle.snapshot();
        let raw_target = match selection {
            RowId::Group(node) => Some(node.clone()),
            RowId::AgentSession(node) => Some(node.clone()),
            RowId::AgentSessionMuxCandidate { mux, .. } => Some(mux.clone()),
            RowId::MuxSession(node) => Some(node.clone()),
            RowId::Pr(node) => Some(node.clone()),
            RowId::Fork(node) => Some(node.clone()),
            RowId::Pin { pin_id } => Some(NodeId::Pin(PinId::new(pin_id.clone()))),
            RowId::Synthetic(_) => None,
        };
        let Some(raw_target) = raw_target else {
            self.explorer = None;
            return;
        };
        // Placeholder pin rows route through here whether they live in
        // the sessions view (`RowId::Pin`) or the mux view (`RowId::
        // MuxSession(NodeId::Pin(...))`). Either way the raw target is a
        // Pin node; redirect to a view-aligned upgrade (last_session in
        // the sessions view, bound / stale mux in the mux view) when one
        // is known, and fall back to the Pin itself otherwise.
        let target = match &raw_target {
            NodeId::Pin(pin) => placeholder_detail_target(snapshot, &pin.id, self.active_view),
            _ => raw_target,
        };
        let home = home_for_config(&self.config);
        // When the placeholder fell back to the Pin node (no view-aligned
        // session or mux known), strip the candidate-link summaries so the
        // right pane reflects the operator-facing reality: the mux isn't
        // running and the session has not been created yet. Resolved /
        // diagnostic surfaces stay so the operator can still see why the
        // pin is in this state.
        let strip_pin_relationships = matches!(&target, NodeId::Pin(_));
        let mut detail = build_node_detail(DetailInputs {
            snapshot,
            target: &target,
            home: home.as_deref(),
        });
        if strip_pin_relationships && let Some(detail) = detail.as_mut() {
            detail.outgoing_links.clear();
            detail.incoming_links.clear();
            detail.resolved.clear();
        }
        self.detail = detail;
        self.recompute_explorer_for(target, home.as_deref(), strip_pin_relationships);
    }

    pub(super) fn recompute_explorer_for(
        &mut self,
        target: NodeId,
        home: Option<&std::path::Path>,
        strip_relationships: bool,
    ) {
        let handle = self.handle.as_ref();
        let Some(handle) = handle else {
            self.explorer = None;
            return;
        };
        let view = build_node_view(ExplorerInputs {
            snapshot: handle.snapshot(),
            target: &target,
            home,
            now: crate::tui::rows::tree_current_unix_epoch(),
        });
        match view {
            None => self.explorer = None,
            Some(mut view) => {
                if strip_relationships {
                    view.relationships.groups.clear();
                }
                // When the focused node hasn't changed, preserve
                // cursor / expansion / breadcrumb across refresh.
                let preserved = self.explorer.as_ref().and_then(|state| {
                    if state.view.focused == target {
                        Some(state.clone())
                    } else {
                        None
                    }
                });
                match preserved {
                    Some(mut state) => {
                        let prev_key = state.selected_row().map(|row| row.key(&state.view));
                        // Replace the view while keeping cursor /
                        // Other-zone expansion / breadcrumb
                        // identity. ADR 0074 §6: the Other zone is
                        // either open or closed; there is no
                        // per-group expansion state to invalidate
                        // when the view rebuilds.
                        state.view = view;
                        if !state.view.has_other_rows() {
                            state.other_expanded = false;
                        }
                        state.reseat_cursor(prev_key);
                        self.explorer = Some(state);
                    }
                    None => {
                        self.explorer = Some(ExplorerState::new(view));
                    }
                }
            }
        }
    }

    pub(super) fn explorer_move_cursor(&mut self, delta: i32) {
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let rows = state.rows();
        if rows.is_empty() {
            state.cursor = 0;
            return;
        }
        state.cursor = crate::tui::cursor::clamp_step(state.cursor, rows.len(), delta);
        self.status_message = None;
    }

    /// Snap the explorer cursor to `index`, clamped to the current
    /// row count. `usize::MAX` is the convention for "last row" so
    /// callers can ask for End without needing to recompute row
    /// counts themselves; this mirrors how `move_selection_to` on
    /// the left-pane tree handles `Msg::End`.
    pub(super) fn explorer_jump_cursor_to(&mut self, index: usize) {
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let rows = state.rows();
        if rows.is_empty() {
            state.cursor = 0;
            return;
        }
        state.cursor = index.min(rows.len() - 1);
        self.status_message = None;
    }

    pub(super) fn explorer_toggle_group(&mut self) {
        // ADR 0074 renamed the surface from "toggle group" to
        // "toggle Other zone." The reducer message identifier
        // (`Msg::ExplorerToggleGroup`) is kept as-is so keybindings
        // and external callers do not churn; the behavior is
        // adjusted to flip the Other-zone visibility when the
        // cursor sits on the `Other` header.
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let rows = state.rows();
        let Some(row) = rows.get(state.cursor).cloned() else {
            return;
        };
        if !row.is_other_header() {
            self.status_message = Some(
                "explorer: nothing to expand here — select the `Other` header to toggle alternates"
                    .to_string(),
            );
            return;
        }
        let prev_key = state.selected_row().map(|row| row.key(&state.view));
        state.other_expanded = !state.other_expanded;
        state.reseat_cursor(prev_key);
        self.status_message = None;
    }

    pub(super) fn toggle_edge_meta(&mut self) {
        self.edge_meta_visible = !self.edge_meta_visible;
        self.status_message = Some(
            if self.edge_meta_visible {
                "explorer: edge meta visible (provenance · confidence · state)"
            } else {
                "explorer: edge meta hidden"
            }
            .to_string(),
        );
    }

    pub(super) fn explorer_toggle_full_detail(&mut self) {
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let prev_key = state.selected_row().map(|row| row.key(&state.view));
        let core_len = state.view.core_fields.len();
        let all_len = state.view.all_fields.len();
        state.full_detail_expanded = !state.full_detail_expanded;
        state.reseat_cursor(prev_key);
        // Render a soft status hint for the no-op case so the
        // operator knows their toggle was received but the node
        // kind doesn't carry extras to expand.
        if core_len == all_len {
            self.status_message = Some(format!(
                "explorer: this node kind has no extra fields ({core_len} total)"
            ));
        } else {
            self.status_message = None;
        }
    }

    pub(super) fn explorer_activate(&mut self) {
        let Some(state) = self.explorer.as_ref() else {
            return;
        };
        let rows = state.rows();
        let Some(row) = rows.get(state.cursor).cloned() else {
            return;
        };
        match row {
            ExplorerRow::OtherHeader { .. } => {
                self.explorer_toggle_group();
            }
            ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. } => {
                if let Some(target) = state.view.drill_target(&row) {
                    self.explorer_drill_into(target);
                }
            }
            ExplorerRow::OtherUnresolved { .. } => {
                self.status_message =
                    Some("explorer: unresolved evidence — `o` opens detail (T8-032)".to_string());
            }
            ExplorerRow::NodeField { .. } => {
                self.status_message = None;
            }
        }
    }

    pub(super) fn explorer_drill_into(&mut self, target: NodeId) {
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let prev_focused = state.view.focused.clone();
        let prev_short_label = state.view.short_label.clone();
        let prev_cursor_key = state.selected_row().map(|row| row.key(&state.view));
        let prev_other_expanded = state.other_expanded;
        let prev_full_detail_expanded = state.full_detail_expanded;
        let prev_left_pane_selection = self.selection.clone();
        let hop = BreadcrumbHop {
            focused: prev_focused,
            short_label: prev_short_label,
            cursor_key: prev_cursor_key,
            other_expanded: prev_other_expanded,
            full_detail_expanded: prev_full_detail_expanded,
            left_pane_selection: prev_left_pane_selection,
        };
        // Build the new view. If we can't load it, leave state alone
        // and surface a status message.
        let home = home_for_config(&self.config);
        let handle = self.handle.as_ref();
        let Some(handle) = handle else {
            return;
        };
        let next = build_node_view(ExplorerInputs {
            snapshot: handle.snapshot(),
            target: &target,
            home: home.as_deref(),
            now: crate::tui::rows::tree_current_unix_epoch(),
        });
        match next {
            None => {
                self.status_message = Some(format!(
                    "explorer: drill target {target:?} not in current snapshot"
                ));
            }
            Some(view) => {
                let mut new_state = ExplorerState::new(view);
                // Carry the breadcrumb stack forward so deep
                // drills accumulate.
                new_state.breadcrumb = state.breadcrumb.clone();
                new_state.breadcrumb.push(hop);
                self.explorer = Some(new_state);
                // Recompute the legacy detail too so the renderer
                // surfaces consistent info during the renderer
                // transition.
                self.detail = build_node_detail(DetailInputs {
                    snapshot: handle.snapshot(),
                    target: &target,
                    home: home.as_deref(),
                });
                self.preview_scroll = 0;
                self.status_message = None;
                // Mirror sync — when the drilled neighbor
                // has a row in the current view, scroll the left
                // tree to it and expand any ancestor groups. When
                // it doesn't, leave the left selection untouched so
                // the operator's prior position is preserved.
                self.mirror_left_pane_to(&target);
            }
        }
    }

    /// Move the left-pane selection to the row corresponding to
    /// `target`, expanding any ancestor group rows along the path.
    /// No-op when the focused node isn't represented in the current
    /// view's row tree.
    pub(super) fn mirror_left_pane_to(&mut self, target: &NodeId) {
        let Some(row_index) = self
            .tree
            .rows
            .iter()
            .position(|row| row_matches(row, target))
        else {
            return;
        };
        let target_id = self.tree.rows[row_index].id.clone();
        let target_depth = self.tree.rows[row_index].depth;
        // Walk back through the flat tree expanding any ancestor
        // group rows at lower depths so the target row is
        // materialized in the rendered visible-rows pass.
        let mut current_depth = target_depth;
        for ancestor_idx in (0..row_index).rev() {
            if current_depth == 0 {
                break;
            }
            let ancestor = &self.tree.rows[ancestor_idx];
            if ancestor.depth < current_depth && ancestor.expandable {
                self.expanded.insert(ancestor.id.clone());
                current_depth = ancestor.depth;
            }
        }
        // Bypass `set_selection` so we don't trigger the
        // recompute_detail path — the explorer was just rebuilt for
        // the drilled neighbor and that's what we want to keep.
        self.selection = Some(target_id);
    }

    pub(super) fn explorer_back(&mut self) {
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let Some(hop) = state.breadcrumb.pop() else {
            // Treat Backspace as a general "go back" gesture, but
            // require a two-press confirmation before backing out of
            // the right pane entirely: the first press surfaces a
            // hint and arms the focus shift, the second performs it.
            // Left-pane Backspace keeps the original status hint and
            // doesn't shift focus.
            if matches!(self.focus, Focus::Right) {
                if self.explorer_back_armed {
                    self.focus = Focus::Left;
                    self.explorer_back_armed = false;
                    self.status_message = None;
                } else {
                    self.status_message = Some(
                        "explorer: no drill history — press Backspace again to return to the left pane"
                            .to_string(),
                    );
                    self.explorer_back_armed = true;
                }
            } else {
                self.status_message = Some("explorer: no drill history to back out of".to_string());
            }
            return;
        };
        let home = home_for_config(&self.config);
        let handle = self.handle.as_ref();
        let Some(handle) = handle else {
            return;
        };
        let view = build_node_view(ExplorerInputs {
            snapshot: handle.snapshot(),
            target: &hop.focused,
            home: home.as_deref(),
            now: crate::tui::rows::tree_current_unix_epoch(),
        });
        let Some(view) = view else {
            self.status_message = Some(
                "explorer: cannot restore breadcrumb hop — node missing from snapshot".to_string(),
            );
            return;
        };
        let breadcrumb_remaining = state.breadcrumb.clone();
        let mut restored = ExplorerState::new(view);
        restored.other_expanded = hop.other_expanded;
        restored.breadcrumb = breadcrumb_remaining;
        restored.full_detail_expanded = hop.full_detail_expanded;
        restored.reseat_cursor(hop.cursor_key);
        self.detail = build_node_detail(DetailInputs {
            snapshot: handle.snapshot(),
            target: &hop.focused,
            home: home.as_deref(),
        });
        self.explorer = Some(restored);
        self.preview_scroll = 0;
        self.status_message = None;
        // Restore the left-pane selection that was active
        // at the time of the drill, so Backspace unwinds both panes
        // together. Bypasses `set_selection` to avoid rebuilding
        // the explorer we just restored.
        if let Some(prev_selection) = hop.left_pane_selection
            && self.tree.rows.iter().any(|row| row.id == prev_selection)
        {
            self.selection = Some(prev_selection);
        }
    }
}
