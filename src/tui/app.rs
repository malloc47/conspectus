//! Pure app state.
//!
//! Per ADR 0024, [`App::update`] is synchronous and free of I/O: the
//! runtime translates terminal events (and, later, background-task
//! results and timer ticks) into [`Msg`]s and feeds them in.
//!
//! v1 milestones layer in incrementally:
//!
//! - `P8-003`: shell scaffold + `Msg::Quit`.
//! - `P8-005`: detail view-model wiring.
//! - `P8-006` (this story): the selection / focus / navigation
//!   state machine — row tree storage, expanded-set, selection
//!   retention across refreshes, panel focus, preview scroll.
//! - `P8-008` onward: background data loader dispatches
//!   `Msg::SetData` results into the reducer.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::model::GraphSnapshot;
use crate::tui::RunConfig;
use crate::tui::detail::{DetailInputs, NodeDetail, build_node_detail};
use crate::tui::rows::{Row, RowId, RowKind, RowTree};

/// Top-level state. Owns the resolved run configuration plus the
/// per-frame UI state.
#[derive(Debug)]
pub struct App {
    config: RunConfig,
    should_quit: bool,
    /// Latest discovery snapshot, wrapped in [`Arc`] so refresh
    /// hand-off is cheap. `None` before the first `SetData`.
    snapshot: Option<Arc<GraphSnapshot>>,
    /// Latest row tree built from `snapshot`. Empty until `SetData`
    /// arrives.
    tree: RowTree,
    /// Set of row ids whose children are visible. Group rows that
    /// the reducer expands on first sight land here too.
    expanded: BTreeSet<RowId>,
    /// Selected row by id. `None` when the tree is empty.
    selection: Option<RowId>,
    /// Detail view-model for the current selection. Recomputed
    /// whenever selection or snapshot changes; the renderer reads
    /// it directly.
    detail: Option<NodeDetail>,
    /// Which panel currently consumes navigation keys.
    focus: Focus,
    /// Vertical scroll offset for the right-panel preview, in
    /// rendered rows.
    preview_scroll: u16,
    /// Unix-epoch seconds at which the current snapshot was
    /// loaded. `None` before the first `SetData`. The renderer
    /// turns this into the header's `updated Ns ago` indicator.
    loaded_at_epoch: Option<i64>,
}

/// Which panel currently consumes navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Left,
    Right,
}

/// Every event the reducer can process. Keep variants narrow and
/// add as stories land; do not make the enum a kitchen sink.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    /// Operator asked to exit (`q`, Ctrl-C, fatal-error
    /// translations).
    Quit,
    /// Background data loader produced a new snapshot + row tree.
    /// The reducer retains current selection by `RowId` when the
    /// same id is present in the new tree, otherwise it snaps to
    /// the nearest visible row by index. `loaded_at_epoch` is the
    /// Unix-epoch second at which the snapshot completed loading,
    /// used by the header's `updated Ns ago` indicator.
    SetData {
        snapshot: Arc<GraphSnapshot>,
        tree: RowTree,
        loaded_at_epoch: i64,
    },
    /// Left panel: move selection down/up one visible row.
    NavDown,
    NavUp,
    /// Left panel: page through visible rows. The runtime passes
    /// the rendered viewport height so the reducer can decide how
    /// many rows constitute one page. Pass 1 if unknown.
    PageDown(u16),
    PageUp(u16),
    /// Left panel: first/last visible row.
    Home,
    End,
    /// Left panel: expand/collapse the selected row. No-op on a
    /// leaf row.
    ToggleExpand,
    /// Move keyboard focus to the next panel.
    CycleFocus,
    /// Right panel: scroll preview by one row.
    ScrollPreviewDown,
    ScrollPreviewUp,
}

impl App {
    /// Build a fresh app at the start of the run.
    pub fn new(config: RunConfig) -> Self {
        Self {
            config,
            should_quit: false,
            snapshot: None,
            tree: RowTree::default(),
            expanded: BTreeSet::new(),
            selection: None,
            detail: None,
            focus: Focus::Left,
            preview_scroll: 0,
            loaded_at_epoch: None,
        }
    }

    /// Read-only access to the immutable run config.
    pub fn config(&self) -> &RunConfig {
        &self.config
    }

    /// True once the runtime should leave the event loop.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Current row tree. Empty before the first `SetData`.
    pub fn tree(&self) -> &RowTree {
        &self.tree
    }

    /// Current selection, `None` before the first `SetData` or when
    /// the tree is empty.
    pub fn selection(&self) -> Option<&RowId> {
        self.selection.as_ref()
    }

    /// Detail view-model for the current selection.
    pub fn detail(&self) -> Option<&NodeDetail> {
        self.detail.as_ref()
    }

    /// Which panel currently has focus.
    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// True when the row is currently expanded.
    pub fn is_expanded(&self, id: &RowId) -> bool {
        self.expanded.contains(id)
    }

    /// Preview scroll offset for the right panel.
    pub fn preview_scroll(&self) -> u16 {
        self.preview_scroll
    }

    /// Latest snapshot, if loaded. Mostly useful to other modules
    /// that compute view-models against the same data.
    pub fn snapshot(&self) -> Option<&Arc<GraphSnapshot>> {
        self.snapshot.as_ref()
    }

    /// Unix-epoch seconds at which the latest snapshot was loaded.
    /// `None` until the first `SetData` arrives.
    pub fn loaded_at_epoch(&self) -> Option<i64> {
        self.loaded_at_epoch
    }

    /// Iterate the row tree, skipping rows whose ancestors are
    /// collapsed. Iteration order matches display order.
    pub fn visible_rows(&self) -> Vec<&Row> {
        let mut out = Vec::with_capacity(self.tree.rows.len());
        // Track the depth of the most recent collapsed ancestor;
        // any row deeper than that is hidden.
        let mut hidden_below: Option<u8> = None;
        for row in &self.tree.rows {
            if let Some(d) = hidden_below
                && row.depth > d
            {
                continue;
            }
            hidden_below = None;
            out.push(row);
            if row.expandable && !self.expanded.contains(&row.id) {
                hidden_below = Some(row.depth);
            }
        }
        out
    }

    /// Apply a single [`Msg`] to the state. Pure: no I/O, no panics,
    /// no clock reads.
    pub fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Quit => self.should_quit = true,
            Msg::SetData {
                snapshot,
                tree,
                loaded_at_epoch,
            } => self.set_data(snapshot, tree, loaded_at_epoch),
            Msg::NavDown => self.move_selection(1),
            Msg::NavUp => self.move_selection(-1),
            Msg::PageDown(viewport) => self.move_selection(i32::from(viewport.max(1))),
            Msg::PageUp(viewport) => self.move_selection(-i32::from(viewport.max(1))),
            Msg::Home => self.move_selection_to(0),
            Msg::End => self.move_selection_to(usize::MAX),
            Msg::ToggleExpand => self.toggle_expand_selected(),
            Msg::CycleFocus => {
                self.focus = match self.focus {
                    Focus::Left => Focus::Right,
                    Focus::Right => Focus::Left,
                };
            }
            Msg::ScrollPreviewDown => {
                self.preview_scroll = self.preview_scroll.saturating_add(1);
            }
            Msg::ScrollPreviewUp => {
                self.preview_scroll = self.preview_scroll.saturating_sub(1);
            }
        }
    }

    fn set_data(&mut self, snapshot: Arc<GraphSnapshot>, tree: RowTree, loaded_at_epoch: i64) {
        self.loaded_at_epoch = Some(loaded_at_epoch);
        // Auto-expand every group row on first arrival of a tree
        // segment so the operator sees their sessions immediately.
        // Already-expanded rows are kept expanded; collapsed rows
        // the user explicitly closed stay closed.
        for row in &tree.rows {
            if row.expandable
                && matches!(row.kind, RowKind::Group(_))
                && !self.expanded.contains(&row.id)
                && !self.tree.rows.iter().any(|r| r.id == row.id)
            {
                self.expanded.insert(row.id.clone());
            }
        }

        let prev_selection = self.selection.take();
        let prev_visible_index = prev_selection
            .as_ref()
            .and_then(|id| self.visible_rows().iter().position(|r| &r.id == id));
        self.snapshot = Some(snapshot);
        self.tree = tree;

        let visible = self.visible_rows_owned();
        if visible.is_empty() {
            self.selection = None;
        } else if let Some(prev) = prev_selection.as_ref()
            && let Some(pos) = visible.iter().position(|id| id == prev)
        {
            self.selection = Some(visible[pos].clone());
        } else if let Some(prev_index) = prev_visible_index {
            let clamped = prev_index.min(visible.len() - 1);
            self.selection = Some(visible[clamped].clone());
        } else {
            self.selection = Some(visible[0].clone());
        }
        self.recompute_detail();
    }

    fn move_selection(&mut self, delta: i32) {
        let visible = self.visible_rows_owned();
        if visible.is_empty() {
            self.selection = None;
            self.detail = None;
            return;
        }
        let current = self
            .selection
            .as_ref()
            .and_then(|id| visible.iter().position(|v| v == id))
            .unwrap_or(0);
        let len = visible.len() as i32;
        let target = (current as i32 + delta).clamp(0, len - 1) as usize;
        self.selection = Some(visible[target].clone());
        self.recompute_detail();
    }

    fn move_selection_to(&mut self, index: usize) {
        let visible = self.visible_rows_owned();
        if visible.is_empty() {
            self.selection = None;
            self.detail = None;
            return;
        }
        let clamped = index.min(visible.len() - 1);
        self.selection = Some(visible[clamped].clone());
        self.recompute_detail();
    }

    fn toggle_expand_selected(&mut self) {
        let Some(id) = self.selection.as_ref().cloned() else {
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

    fn visible_rows_owned(&self) -> Vec<RowId> {
        self.visible_rows().iter().map(|r| r.id.clone()).collect()
    }

    fn recompute_detail(&mut self) {
        self.detail = None;
        self.preview_scroll = 0;
        let Some(selection) = self.selection.as_ref() else {
            return;
        };
        let target = match selection {
            RowId::Group(node) => Some(node.clone()),
            RowId::AgentSession(node) => Some(node.clone()),
            RowId::AgentSessionMuxCandidate { mux, .. } => Some(mux.clone()),
            RowId::MuxSession(node) => Some(node.clone()),
            RowId::Pr(node) => Some(node.clone()),
            RowId::Fork(node) => Some(node.clone()),
            RowId::Synthetic(_) => None,
        };
        let Some(target) = target else {
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        self.detail = build_node_detail(DetailInputs {
            snapshot: snapshot.as_ref(),
            target: &target,
            home: home_for_config(&self.config),
        });
    }
}

fn home_for_config(_config: &RunConfig) -> Option<&std::path::Path> {
    // RunConfig doesn't carry the home directory today; the
    // dispatcher passes paths in already-shortened form via the
    // row tree, and the detail builder shortens its own when given
    // a home. For now we read the environment lazily — pure-state
    // tests can patch this when home-sensitive behavior matters.
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, GraphNode, GraphSnapshot, RepoId, RepoNode, WorktreeId,
        WorktreeNode,
    };
    use crate::resolve::resolve_snapshot;
    use crate::tui::SessionsGrouping;
    use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};

    fn make_snapshot_with(sessions: &[(&str, &str, &str)]) -> GraphSnapshot {
        let mut snap = GraphSnapshot::empty();
        // Unique repo per session for simplicity.
        for (harness, key, cwd) in sessions {
            let repo_id = RepoId::new(*cwd);
            snap.nodes
                .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
            snap.nodes.push(GraphNode::Worktree(WorktreeNode {
                id: WorktreeId::new(repo_id, cwd.to_string()),
                root: cwd.to_string(),
                git_dir: None,
                current_branch: None,
            }));
            snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new(*harness, "/state", *key),
                harness_key: harness.to_string(),
                cwd: Some(cwd.to_string()),
                title: None,
                last_message_preview: None,
            }));
        }
        resolve_snapshot(snap)
    }

    fn build_tree(snap: &GraphSnapshot) -> RowTree {
        build_sessions_tree(SessionsBuildInputs {
            snapshot: snap,
            grouping: SessionsGrouping::Graph,
            home: None,
            now: None,
        })
    }

    fn seeded_app(sessions: &[(&str, &str, &str)]) -> App {
        let snap = Arc::new(make_snapshot_with(sessions));
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: snap,
            tree,
            loaded_at_epoch: 1_700_000_000,
        });
        app
    }

    #[test]
    fn empty_tree_leaves_selection_none() {
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: Arc::new(GraphSnapshot::empty()),
            tree: RowTree::default(),
            loaded_at_epoch: 1_700_000_000,
        });
        assert!(app.selection().is_none());
        assert!(app.detail().is_none());
    }

    #[test]
    fn set_data_default_expands_group_rows_and_selects_first_visible() {
        let app = seeded_app(&[("codex", "a", "/p/proj")]);
        let visible = app.visible_rows();
        assert!(!visible.is_empty(), "groups auto-expanded");
        assert!(app.selection().is_some());
    }

    #[test]
    fn nav_down_and_up_walk_visible_rows() {
        let app = seeded_app(&[
            ("codex", "a", "/p/proja"),
            ("codex", "b", "/p/projb"),
            ("codex", "c", "/p/projc"),
        ]);
        let mut app = app;
        let visible = app.visible_rows_owned();
        assert!(visible.len() >= 3);

        let start = app.selection().cloned().unwrap();
        app.update(Msg::NavDown);
        let after_down = app.selection().cloned().unwrap();
        assert_ne!(start, after_down);
        app.update(Msg::NavUp);
        assert_eq!(app.selection().cloned().unwrap(), start);
    }

    #[test]
    fn end_jumps_to_last_visible_and_home_returns_to_first() {
        let app = seeded_app(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
        let mut app = app;
        let visible = app.visible_rows_owned();
        let first = visible.first().cloned().unwrap();
        let last = visible.last().cloned().unwrap();

        app.update(Msg::End);
        assert_eq!(app.selection().cloned().unwrap(), last);
        app.update(Msg::Home);
        assert_eq!(app.selection().cloned().unwrap(), first);
    }

    #[test]
    fn toggle_expand_collapses_and_re_expands_groups() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        // Selection starts on the first row (a group row).
        let group_id = app.selection().cloned().unwrap();
        assert!(matches!(group_id, RowId::Group(_)));
        let visible_before = app.visible_rows_owned().len();
        app.update(Msg::ToggleExpand);
        let visible_collapsed = app.visible_rows_owned().len();
        assert!(
            visible_collapsed < visible_before,
            "collapsing hides descendants"
        );
        app.update(Msg::ToggleExpand);
        let visible_again = app.visible_rows_owned().len();
        assert_eq!(
            visible_again, visible_before,
            "re-expansion restores the view"
        );
    }

    #[test]
    fn cycle_focus_alternates_panels() {
        let mut app = App::new(RunConfig::defaults());
        assert_eq!(app.focus(), Focus::Left);
        app.update(Msg::CycleFocus);
        assert_eq!(app.focus(), Focus::Right);
        app.update(Msg::CycleFocus);
        assert_eq!(app.focus(), Focus::Left);
    }

    #[test]
    fn scroll_preview_clamps_at_zero() {
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::ScrollPreviewDown);
        app.update(Msg::ScrollPreviewDown);
        assert_eq!(app.preview_scroll(), 2);
        app.update(Msg::ScrollPreviewUp);
        app.update(Msg::ScrollPreviewUp);
        app.update(Msg::ScrollPreviewUp);
        assert_eq!(app.preview_scroll(), 0);
    }

    #[test]
    fn set_data_retains_selection_by_row_id_when_present() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
        // Move to a session row (not a group) so the retained
        // selection is clearly tied to the agent session.
        app.update(Msg::End);
        let saved = app.selection().cloned().unwrap();

        // Rebuild from the same snapshot; the row tree is
        // deterministic, so RowId equality should retain selection.
        let snap = app.snapshot.clone().unwrap();
        let tree = build_tree(&snap);
        app.update(Msg::SetData {
            snapshot: snap,
            tree,
            loaded_at_epoch: 1_700_000_000,
        });
        assert_eq!(app.selection().cloned().unwrap(), saved);
    }

    #[test]
    fn set_data_falls_back_to_nearest_index_when_selection_disappears() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
        app.update(Msg::End);
        let original_selection = app.selection().cloned().unwrap();

        // Build a snapshot that drops the previously-selected row.
        let snap = Arc::new(make_snapshot_with(&[("codex", "a", "/p/proja")]));
        let tree = build_tree(&snap);
        app.update(Msg::SetData {
            snapshot: snap,
            tree,
            loaded_at_epoch: 1_700_000_000,
        });

        let new_selection = app.selection().cloned().unwrap();
        assert_ne!(new_selection, original_selection);
        assert!(
            app.visible_rows_owned()
                .iter()
                .any(|id| id == &new_selection),
            "fallback selection must be visible"
        );
    }

    #[test]
    fn detail_is_recomputed_when_selection_lands_on_a_node_row() {
        let app = seeded_app(&[("codex", "a", "/p/proja")]);
        // Selection auto-lands on the first visible row (a group);
        // groups still produce a NodeDetail because they're backed
        // by a NodeId.
        assert!(app.detail().is_some());
    }
}
