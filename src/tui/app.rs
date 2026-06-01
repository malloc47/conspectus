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

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use crate::model::{MuxSessionId, NodeId};
use crate::tui::detail::{NodeDetail, build_node_detail_from_conn};
use crate::tui::explorer::{
    BreadcrumbHop, ExplorerRow, ExplorerRowKey, GroupKey, NodeView, build_node_view_from_conn,
};
use crate::tui::preview::{PreviewContent, PreviewEntry, PreviewStore};
use crate::tui::rows::{Row, RowId, RowKind, RowTree};
use crate::tui::{RunConfig, View};

pub struct GraphDb(Rc<rusqlite::Connection>);

impl GraphDb {
    pub(crate) fn new(conn: rusqlite::Connection) -> Self {
        Self(Rc::new(conn))
    }

    #[cfg(test)]
    pub(crate) fn from_snapshot(snapshot: &crate::model::GraphSnapshot) -> Self {
        let conn = crate::query::materialize_snapshot(snapshot).expect("materialize TUI snapshot");
        Self(Rc::new(conn))
    }

    pub(crate) fn conn(&self) -> &rusqlite::Connection {
        &self.0
    }
}

impl Clone for GraphDb {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl fmt::Debug for GraphDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("GraphDb").field(&"<sqlite>").finish()
    }
}

impl PartialEq for GraphDb {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Per-provider availability status for the right-side status-bar
/// chips (T8-003, Phase 8 error-state table).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderStatus {
    pub tmux_disabled: bool,
    pub tmux_available: Option<bool>,
    pub tmux_reason: Option<String>,
    pub forge_disabled: bool,
    pub forge_available: Option<bool>,
    pub forge_reason: Option<String>,
}

impl ProviderStatus {
    pub fn has_any_chip(&self) -> bool {
        self.tmux_disabled
            || !self.tmux_available.unwrap_or(true)
            || self.forge_disabled
            || !self.forge_available.unwrap_or(true)
    }
}

/// Top-level state. Owns the resolved run configuration plus the
/// per-frame UI state.
#[derive(Debug)]
pub struct App {
    config: RunConfig,
    should_quit: bool,
    /// Latest materialized graph database. `None` before the first
    /// `SetData`.
    database: Option<GraphDb>,
    /// Latest row tree built from `snapshot`. Empty until `SetData`
    /// arrives.
    tree: RowTree,
    /// Set of row ids whose children are visible. Group rows that
    /// the reducer expands on first sight land here too.
    expanded: BTreeSet<RowId>,
    /// Selected row by id. `None` when the tree is empty.
    selection: Option<RowId>,
    /// Detail view-model for the current selection. Recomputed
    /// whenever selection or database changes; the renderer reads
    /// it directly.
    detail: Option<NodeDetail>,
    /// Whether linked-entity summary rows in the right-panel detail
    /// are expanded in place.
    detail_links_expanded: bool,
    /// Right-panel graph explorer state (T8-027 / T8-028). Carries
    /// the focused node view, the navigation cursor, group expansion,
    /// and the breadcrumb stack for drilldown. `None` until the
    /// reducer has resolved a selection into a node view.
    explorer: Option<ExplorerState>,
    /// Which panel currently consumes navigation keys.
    focus: Focus,
    /// Vertical scroll offset for the right-panel preview, in
    /// rendered rows.
    preview_scroll: u16,
    /// Unix-epoch seconds at which the current snapshot was
    /// loaded. `None` before the first `SetData`. The renderer
    /// turns this into the header's `updated Ns ago` indicator.
    loaded_at_epoch: Option<i64>,
    /// Transient status-bar message, e.g. the "disabled because…"
    /// reason for a key that didn't apply to the current selection.
    /// Cleared on the next selection / focus change.
    status_message: Option<String>,
    /// Provider toggles and availability surfaced as status-bar
    /// chips per the Phase 8 error-state table. Populated by the
    /// runtime from discovery diagnostics and env-var toggles.
    provider_status: ProviderStatus,
    /// Human-readable reason for the most recent refresh failure.
    /// `None` when the last refresh succeeded (or on first launch).
    /// The status bar renders this as a stale/error marker.
    refresh_failure: Option<String>,
    /// Cache of recent tmux pane captures, keyed by mux id. The
    /// renderer reads this for the right-panel preview when the
    /// selection points at a muxed agent session or a mux node.
    preview_store: PreviewStore,
    /// Left-panel vertical scroll offset, in rendered lines. The
    /// renderer reads/updates this via [`App::adjust_left_scroll`]
    /// each frame so the selected row stays visible without the
    /// renderer needing `&mut self`.
    left_scroll: Cell<u16>,
    /// Active rename overlay state per ADR 0029 / ADR 0030. `None`
    /// when no overlay is open; `Some` suspends the surrounding
    /// keymap and routes input through the modal.
    rename_overlay: Option<crate::tui::widgets::input::TextInputState>,
    /// Active controls overlay (ADR 0031, F8-004). `None` when the
    /// overlay is closed; `Some` suspends the surrounding keymap
    /// and routes input through the modal.
    controls_overlay: Option<crate::tui::widgets::controls::ControlsOverlayState>,
    /// Active `/` search overlay (T8-017). `None` when closed;
    /// `Some` suspends the surrounding keymap, routes input
    /// through the modal, and overlays a ranked match list within
    /// the active filter set.
    search_overlay: Option<crate::tui::widgets::search::SearchOverlayState>,
    /// Active `?` help overlay (F8-011). `None` when closed.
    help_overlay: Option<crate::tui::widgets::help::HelpOverlayState>,
    /// Active `o` full-value modal (T8-030). `None` when closed;
    /// `Some` suspends navigation keys and routes input through the
    /// modal.
    value_modal: Option<crate::tui::widgets::value_modal::ValueModalState>,
    /// Global sort toggle (ADR 0031). Per-view state covers
    /// filter/grouping/expanded; sort stays global because the
    /// recency-vs-hierarchy choice is view-independent in operator
    /// practice. Seeded from [`RunConfig::default_sort`].
    sort: super::Sort,
    /// Active row filter (ADR 0031, F8-003). Mirrors the active
    /// view's slot in `view_states` so callers don't pay a map
    /// lookup per read. Kept in sync via `switch_to_view` /
    /// `apply_controls_action::SetFilter`.
    filter: crate::filter::RowFilter,
    /// Active grouping (ADR 0031, F8-003). Same caching pattern as
    /// `filter` — mirrors the active view's slot.
    grouping: super::Grouping,
    /// Saved UI state for views the operator is not currently
    /// looking at (ADR 0031, F8-003). On view switch the active
    /// slot is saved here and the target slot loaded into the
    /// active fields. Sort stays global, so it lives on `App`
    /// rather than per-view.
    view_states: BTreeMap<View, ViewStateSlot>,
}

/// Saved-per-view UI state. Each entry holds everything that needs
/// to round-trip across view switches per ADR 0031: filter,
/// grouping, expanded-row set, selection, and left-panel scroll
/// offset. Constructed lazily — view_states is empty at App::new
/// and slots fill in as the operator visits each view (or saves
/// the currently-active view on the first switch away).
#[derive(Debug, Clone)]
pub(crate) struct ViewStateSlot {
    pub filter: crate::filter::RowFilter,
    pub grouping: super::Grouping,
    pub expanded: BTreeSet<RowId>,
    pub selection: Option<RowId>,
    pub left_scroll: u16,
}

impl ViewStateSlot {
    fn defaults_for(view: View) -> Self {
        Self {
            filter: crate::filter::RowFilter::default(),
            grouping: super::Grouping::default_for(view),
            expanded: BTreeSet::new(),
            selection: None,
            left_scroll: 0,
        }
    }
}

/// Which panel currently consumes navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Left,
    Right,
}

/// Right-pane graph explorer state. Owns the focused node's view
/// model, the cursor position within the flat row list, the set of
/// expanded multi-link groups, and the breadcrumb stack used by
/// drilldown.
#[derive(Debug, Clone)]
pub struct ExplorerState {
    /// View model for the currently-focused node.
    pub view: NodeView,
    /// Index into [`NodeView::flat_rows`] when materialized with the
    /// current `expanded_groups`. Clamped on every update so the
    /// renderer can read it unchecked.
    pub cursor: usize,
    /// Set of multi-link groups whose children are currently visible.
    pub expanded_groups: BTreeSet<GroupKey>,
    /// Drill history. Empty when the focused node is the same one
    /// the left tree points at.
    pub breadcrumb: Vec<BreadcrumbHop>,
}

impl ExplorerState {
    /// Build a fresh state for `view`, starting with the cursor on
    /// the first row. We default to the first row (a Node field)
    /// rather than the first relationship so the Preview zone falls
    /// back to the legacy live preview body — operators expect to
    /// see the tmux pane capture, transcript tail, etc. without
    /// having to scroll down into relationships first. Pressing
    /// `j` walks into relationship rows, at which point the Preview
    /// switches to neighbor + edge content.
    pub fn new(view: NodeView) -> Self {
        let expanded_groups = BTreeSet::new();
        Self {
            view,
            cursor: 0,
            expanded_groups,
            breadcrumb: Vec::new(),
        }
    }

    /// The flat list of selectable rows for the current view +
    /// expansion state. Computed each call rather than cached so the
    /// view model stays the source of truth.
    pub fn rows(&self) -> Vec<ExplorerRow> {
        self.view.flat_rows(&self.expanded_groups)
    }

    /// Selected row, if any.
    pub fn selected_row(&self) -> Option<ExplorerRow> {
        self.rows().into_iter().nth(self.cursor)
    }

    /// Preserve cursor identity across a rebuild by row key. Used
    /// when the underlying view model changes (refresh, group
    /// expand) so the cursor sticks to the same logical row.
    fn reseat_cursor(&mut self, previous_key: Option<ExplorerRowKey>) {
        let rows = self.rows();
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        let target =
            previous_key.and_then(|key| rows.iter().position(|row| row.key(&self.view) == key));
        self.cursor = target.unwrap_or_else(|| {
            // Fall back to the first link or group header so the
            // cursor lands on something actionable.
            rows.iter()
                .position(|row| {
                    matches!(
                        row,
                        ExplorerRow::Link { .. } | ExplorerRow::GroupHeader { .. }
                    )
                })
                .unwrap_or(0)
        });
    }
}

/// Every event the reducer can process. Keep variants narrow and
/// add as stories land; do not make the enum a kitchen sink.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    /// Operator asked to exit (`q`, Ctrl-C, fatal-error
    /// translations).
    Quit,
    /// Background data loader produced a new SQLite graph + row tree.
    /// The reducer retains current selection by `RowId` when the
    /// same id is present in the new tree, otherwise it snaps to
    /// the nearest visible row by index. `loaded_at_epoch` is the
    /// Unix-epoch second at which the snapshot completed loading,
    /// used by the header's `updated Ns ago` indicator.
    /// `initial_selection_hint` is consulted on the *first* SetData
    /// (when no prior selection exists) to pre-select the launch-
    /// context row (T8-013) instead of the leading row in the tree;
    /// later refreshes ignore the hint and prefer the retained
    /// selection.
    SetData {
        snapshot: GraphDb,
        tree: RowTree,
        loaded_at_epoch: i64,
        initial_selection_hint: Option<RowId>,
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
    /// Right panel: expand/collapse linked entity details under the
    /// selected node's compact link rows.
    ToggleLinkedDetails,
    /// Right panel (graph explorer): move the cursor down one row
    /// in the flat row list (T8-028).
    ExplorerNavDown,
    /// Right panel (graph explorer): move the cursor up one row.
    ExplorerNavUp,
    /// Right panel (graph explorer): activate the highlighted row.
    /// On a group header this toggles the group's expansion; on a
    /// link row it drills into the neighbor and pushes a breadcrumb
    /// hop. No-op on unresolved-evidence rows in v1.
    ExplorerActivate,
    /// Right panel (graph explorer): toggle expansion of the
    /// highlighted multi-link group. No-op when the cursor isn't
    /// on a header.
    ExplorerToggleGroup,
    /// Right panel (graph explorer): back out of the most recent
    /// drilldown hop, restoring the previous focused node and the
    /// cursor / expansion state saved with it. No-op when the
    /// breadcrumb stack is empty.
    ExplorerBack,
    /// Move keyboard focus to the next panel.
    CycleFocus,
    /// Right panel: scroll preview by `delta` rows. Positive
    /// scrolls down (deeper into the buffer), negative scrolls
    /// up. The reducer clamps the offset at zero.
    ScrollPreviewBy(i32),
    /// Set or clear the transient status-bar message. `None`
    /// clears any prior message.
    SetStatus(Option<String>),
    /// Store a fresh mux preview capture in the per-mux cache.
    /// The runtime dispatches this after running
    /// `tmux capture-pane` against the selection's mux target.
    SetMuxPreview {
        mux: MuxSessionId,
        content: PreviewContent,
    },
    /// Update the provider availability status surfaced as chips
    /// in the status bar. The runtime populates this from
    /// discovery diagnostics and env-var toggles (T8-003).
    SetProviderStatus(ProviderStatus),
    /// Record that the most recent refresh failed. The previous
    /// good snapshot remains in place; this message surfaces a
    /// stale indicator in the header or status bar (T8-003).
    SetRefreshFailure(String),
}

impl App {
    /// Build a fresh app at the start of the run.
    pub fn new(config: RunConfig) -> Self {
        let mut config = config;
        if matches!(config.sessions_grouping, super::SessionsGrouping::None) {
            config.default_sort = super::Sort::Recency;
        }
        let sort = config.default_sort;
        let filter = config.initial_filter.clone();
        let grouping = super::Grouping::Sessions(config.sessions_grouping);
        Self {
            config,
            should_quit: false,
            database: None,
            tree: RowTree::default(),
            expanded: BTreeSet::new(),
            selection: None,
            detail: None,
            detail_links_expanded: false,
            explorer: None,
            focus: Focus::Left,
            preview_scroll: 0,
            loaded_at_epoch: None,
            status_message: None,
            provider_status: ProviderStatus::default(),
            refresh_failure: None,
            preview_store: PreviewStore::new(),
            left_scroll: Cell::new(0),
            rename_overlay: None,
            controls_overlay: None,
            search_overlay: None,
            help_overlay: None,
            value_modal: None,
            sort,
            filter,
            grouping,
            view_states: BTreeMap::new(),
        }
    }

    /// Snapshot the active view's UI state into a saved slot. Used
    /// by [`Self::switch_to_view`] to preserve filter / grouping /
    /// selection / expanded-set / scroll across view switches.
    fn snapshot_active_state(&self) -> ViewStateSlot {
        ViewStateSlot {
            filter: self.filter.clone(),
            grouping: self.grouping,
            expanded: self.expanded.clone(),
            selection: self.selection.clone(),
            left_scroll: self.left_scroll.get(),
        }
    }

    /// Restore a saved view state into the active fields.
    fn restore_active_state(&mut self, slot: ViewStateSlot) {
        self.filter = slot.filter;
        self.grouping = slot.grouping;
        self.expanded = slot.expanded;
        self.selection = slot.selection;
        self.left_scroll.set(slot.left_scroll);
    }

    /// Switch the active view to `target`, saving the previous
    /// view's state into `view_states` and loading the target's
    /// state (or fresh defaults on first visit). Per ADR 0031 sort
    /// stays global, so callers don't touch it here.
    pub(crate) fn switch_to_view(&mut self, target: View) {
        let from = self.config.default_view;
        if from == target {
            return;
        }
        let saved = self.snapshot_active_state();
        self.view_states.insert(from, saved);
        let loaded = self
            .view_states
            .remove(&target)
            .unwrap_or_else(|| ViewStateSlot::defaults_for(target));
        self.restore_active_state(loaded);
        self.config.default_view = target;
        // Keep `config.sessions_grouping` in sync for the sessions
        // row-tree builder. Other views read their grouping from
        // `self.grouping` once their builders land.
        if let super::Grouping::Sessions(g) = self.grouping {
            self.config.sessions_grouping = g;
        }
        self.force_recency_for_flat_sessions();
        // Mirror the active filter into config so refresh() picks
        // it up when it rebuilds the row tree.
        self.config.initial_filter = self.filter.clone();
        self.status_message = None;
    }

    /// Active rename-overlay state, if any.
    pub fn rename_overlay(&self) -> Option<&crate::tui::widgets::input::TextInputState> {
        self.rename_overlay.as_ref()
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn rename_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::input::TextInputState> {
        self.rename_overlay.as_mut()
    }

    /// Open the rename overlay for `state`. The caller pre-populates
    /// the input with the current alias, harness title, or empty
    /// string per ADR 0030.
    pub fn open_rename_overlay(&mut self, state: crate::tui::widgets::input::TextInputState) {
        self.rename_overlay = Some(state);
    }

    /// Close the rename overlay without committing.
    pub fn close_rename_overlay(&mut self) {
        self.rename_overlay = None;
    }

    /// Active controls-overlay state (ADR 0031, F8-004), if any.
    pub fn controls_overlay(&self) -> Option<&crate::tui::widgets::controls::ControlsOverlayState> {
        self.controls_overlay.as_ref()
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn controls_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::controls::ControlsOverlayState> {
        self.controls_overlay.as_mut()
    }

    /// Open the controls overlay with the cursor on the active view
    /// row.
    pub fn open_controls_overlay(&mut self) {
        let ctx = self.controls_context();
        self.controls_overlay = Some(crate::tui::widgets::controls::ControlsOverlayState::new(
            &ctx,
        ));
    }

    /// Open the controls overlay with the cursor on the Filters >
    /// Harness row. Used by the `f` accelerator (F8-005).
    pub fn open_controls_overlay_at_filters(&mut self) {
        let ctx = self.controls_context();
        self.controls_overlay =
            Some(crate::tui::widgets::controls::ControlsOverlayState::new_at_filters(&ctx));
    }

    /// Close the controls overlay without applying anything.
    pub fn close_controls_overlay(&mut self) {
        self.controls_overlay = None;
    }

    /// Active `/` search overlay (T8-017), if any.
    pub fn search_overlay(&self) -> Option<&crate::tui::widgets::search::SearchOverlayState> {
        self.search_overlay.as_ref()
    }

    pub fn search_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::search::SearchOverlayState> {
        self.search_overlay.as_mut()
    }

    pub fn open_search_overlay(&mut self) {
        self.search_overlay = Some(crate::tui::widgets::search::SearchOverlayState::new());
    }

    pub fn close_search_overlay(&mut self) {
        self.search_overlay = None;
    }

    /// Active `?` help overlay (F8-011), if any.
    pub fn help_overlay(&self) -> Option<&crate::tui::widgets::help::HelpOverlayState> {
        self.help_overlay.as_ref()
    }

    pub fn help_overlay_mut(&mut self) -> Option<&mut crate::tui::widgets::help::HelpOverlayState> {
        self.help_overlay.as_mut()
    }

    pub fn open_help_overlay(&mut self) {
        self.help_overlay = Some(crate::tui::widgets::help::HelpOverlayState::new());
    }

    pub fn close_help_overlay(&mut self) {
        self.help_overlay = None;
    }

    /// Active `o` full-value modal (T8-030), if any.
    pub fn value_modal(&self) -> Option<&crate::tui::widgets::value_modal::ValueModalState> {
        self.value_modal.as_ref()
    }

    pub fn value_modal_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::value_modal::ValueModalState> {
        self.value_modal.as_mut()
    }

    pub fn close_value_modal(&mut self) {
        self.value_modal = None;
    }

    /// Open the full-value modal for whatever value the active
    /// surface points at. The explorer cursor row is the v1
    /// candidate: long node-field values truncate with a `(truncated
    /// · o)` hint and `o` opens the full text here. No-op when no
    /// long value is reachable from the current cursor.
    pub fn open_value_modal_for_cursor(&mut self) {
        // Skip if nothing's mounted.
        let Some(state) = self.explorer.as_ref() else {
            self.status_message = Some(
                "explorer: nothing to open here — `o` opens long values on the cursor row"
                    .to_string(),
            );
            return;
        };
        let Some(row) = state.selected_row() else {
            return;
        };
        let opened = match row {
            ExplorerRow::NodeField { index, label, .. } => {
                state.view.core_fields.get(index).and_then(|field| {
                    field
                        .long_value
                        .clone()
                        .map(|value| (label.to_string(), value))
                })
            }
            _ => None,
        };
        match opened {
            Some((label, value)) => {
                self.value_modal = Some(crate::tui::widgets::value_modal::ValueModalState::new(
                    label, value,
                ));
                self.status_message = None;
            }
            None => {
                self.status_message =
                    Some("explorer: row has no truncated value to open".to_string());
            }
        }
    }

    /// Programmatically set the selection to a row id, recomputing
    /// the detail view-model. Used by the search overlay to land
    /// the operator on a picked match without typing j/k. No-op
    /// when the id isn't currently visible (the overlay would have
    /// rejected it during commit; the guard here is defensive).
    pub fn set_selection(&mut self, id: RowId) {
        if !self.tree.rows.iter().any(|row| row.id == id) {
            return;
        }
        self.selection = Some(id);
        self.status_message = None;
        self.recompute_detail();
    }

    /// Snapshot of the live state the controls overlay renders
    /// against. Borrowed each frame so the overlay never lags
    /// behind the app.
    pub fn controls_context(&self) -> crate::tui::widgets::controls::ControlsContext<'_> {
        crate::tui::widgets::controls::ControlsContext {
            view: self.config.default_view,
            grouping: self.grouping,
            filter: &self.filter,
            sort: self.sort,
        }
    }

    /// Active row filter for the visible view. F8-003 will
    /// generalize this to per-view state.
    pub fn filter(&self) -> &crate::filter::RowFilter {
        &self.filter
    }

    /// Active grouping for the visible view.
    pub fn grouping(&self) -> super::Grouping {
        self.grouping
    }

    /// Global sort order.
    pub fn sort(&self) -> super::Sort {
        self.sort
    }

    /// Apply a [`crate::tui::widgets::controls::ControlsAction`] to
    /// the app state. The runtime calls this when the controls
    /// overlay returns an `ApplyAndStay` / `ApplyAndClose` outcome
    /// so the side effect lives in one place.
    pub fn apply_controls_action(&mut self, action: crate::tui::widgets::controls::ControlsAction) {
        use crate::tui::widgets::controls::ControlsAction;
        match action {
            ControlsAction::SwitchView(view) => {
                // Per ADR 0031 / F8-003: save the prior view's
                // filter / grouping / expanded / selection / scroll
                // into the per-view map and load the target view's
                // saved state (or fresh defaults on first visit).
                self.switch_to_view(view);
            }
            ControlsAction::SetGrouping(g) => {
                self.grouping = g;
                match g {
                    super::Grouping::Sessions(g) => {
                        self.config.sessions_grouping = g;
                        self.force_recency_for_flat_sessions();
                    }
                    super::Grouping::Mux(g) => {
                        self.config.mux_grouping = g;
                    }
                    super::Grouping::Union(_)
                    | super::Grouping::Prs(_)
                    | super::Grouping::Forks(_) => {}
                }
            }
            ControlsAction::SetFilter(filter) => {
                self.filter = filter.clone();
                self.config.initial_filter = filter;
            }
            ControlsAction::SetSort(sort) => {
                let sort = if matches!(
                    self.grouping,
                    super::Grouping::Sessions(super::SessionsGrouping::None)
                ) {
                    super::Sort::Recency
                } else {
                    sort
                };
                self.sort = sort;
                self.config.default_sort = sort;
            }
        }
    }

    fn force_recency_for_flat_sessions(&mut self) {
        if matches!(
            self.grouping,
            super::Grouping::Sessions(super::SessionsGrouping::None)
        ) {
            self.sort = super::Sort::Recency;
            self.config.default_sort = super::Sort::Recency;
        }
    }

    /// Read-only access to the immutable run config.
    pub fn config(&self) -> &RunConfig {
        &self.config
    }

    /// Resolved color theme (ADR 0032). Renderer reads from this in
    /// place of inline color literals.
    pub fn theme(&self) -> &crate::tui::Theme {
        &self.config.theme
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

    pub fn detail_links_expanded(&self) -> bool {
        self.detail_links_expanded
    }

    /// Right-pane graph explorer state for the current focused
    /// node. `None` until the reducer has resolved a selection.
    pub fn explorer(&self) -> Option<&ExplorerState> {
        self.explorer.as_ref()
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

    /// Latest graph database, if loaded. Mostly useful to other modules
    /// that compute view-models against the same data.
    pub(crate) fn graph_db(&self) -> Option<&GraphDb> {
        self.database.as_ref()
    }

    /// Unix-epoch seconds at which the latest snapshot was loaded.
    /// `None` until the first `SetData` arrives.
    pub fn loaded_at_epoch(&self) -> Option<i64> {
        self.loaded_at_epoch
    }

    /// Transient status-bar message, if any. Renderer shows it in
    /// the status zone; the reducer clears it on the next
    /// selection / focus change so messages don't linger.
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    /// Current provider availability status used for right-side
    /// status-bar chips (T8-003).
    pub fn provider_status(&self) -> &ProviderStatus {
        &self.provider_status
    }

    /// Reason for the most recent refresh failure, if any.
    pub fn refresh_failure(&self) -> Option<&str> {
        self.refresh_failure.as_deref()
    }

    /// Look up a cached mux preview. Returns `None` if the mux
    /// hasn't been captured yet.
    pub fn mux_preview(&self, mux: &MuxSessionId) -> Option<&PreviewEntry> {
        self.preview_store.get(mux)
    }

    /// Adjust the left-panel scroll so `selected_line` is visible
    /// inside a viewport of `viewport_height` lines, then return
    /// the resulting offset. Pure with respect to selection — no
    /// reducer state changes — but mutates the per-frame scroll
    /// cell through interior mutability. Called by the renderer
    /// each draw.
    ///
    /// Rules:
    /// - If the selection is above the current top, the top edge
    ///   moves up to bring it into view.
    /// - If the selection is at or below the current bottom, the
    ///   bottom edge moves down (and the offset advances).
    /// - Otherwise the offset is left alone — incidental cursor
    ///   movement inside the viewport doesn't reshuffle the view.
    pub fn adjust_left_scroll(&self, selected_line: usize, viewport_height: u16) -> u16 {
        let vh = viewport_height as usize;
        if vh == 0 {
            return self.left_scroll.get();
        }
        let mut offset = self.left_scroll.get() as usize;
        if selected_line < offset {
            offset = selected_line;
        } else if selected_line >= offset + vh {
            offset = selected_line + 1 - vh;
        }
        let clamped = offset.min(u16::MAX as usize) as u16;
        self.left_scroll.set(clamped);
        clamped
    }

    /// Test-only accessor for the current scroll offset.
    #[cfg(test)]
    pub fn left_scroll(&self) -> u16 {
        self.left_scroll.get()
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
                initial_selection_hint,
            } => self.set_data(snapshot, tree, loaded_at_epoch, initial_selection_hint),
            Msg::NavDown => self.move_selection(1),
            Msg::NavUp => self.move_selection(-1),
            Msg::PageDown(viewport) => self.move_selection(i32::from(viewport.max(1))),
            Msg::PageUp(viewport) => self.move_selection(-i32::from(viewport.max(1))),
            Msg::Home => self.move_selection_to(0),
            Msg::End => self.move_selection_to(usize::MAX),
            Msg::ToggleExpand => self.toggle_expand_selected(),
            Msg::ToggleLinkedDetails => self.toggle_linked_details(),
            Msg::ExplorerNavDown => self.explorer_move_cursor(1),
            Msg::ExplorerNavUp => self.explorer_move_cursor(-1),
            Msg::ExplorerActivate => self.explorer_activate(),
            Msg::ExplorerToggleGroup => self.explorer_toggle_group(),
            Msg::ExplorerBack => self.explorer_back(),
            Msg::CycleFocus => {
                self.focus = match self.focus {
                    Focus::Left => Focus::Right,
                    Focus::Right => Focus::Left,
                };
            }
            Msg::ScrollPreviewBy(delta) => {
                let current = i32::from(self.preview_scroll);
                let next = current.saturating_add(delta).max(0);
                self.preview_scroll =
                    u16::try_from(next.min(i32::from(u16::MAX))).unwrap_or(u16::MAX);
            }
            Msg::SetStatus(message) => {
                self.status_message = message;
            }
            Msg::SetMuxPreview { mux, content } => {
                self.preview_store.insert(mux, content);
            }
            Msg::SetProviderStatus(status) => {
                self.provider_status = status;
            }
            Msg::SetRefreshFailure(reason) => {
                self.refresh_failure = Some(reason);
            }
        }
    }

    fn set_data(
        &mut self,
        snapshot: GraphDb,
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
        self.database = Some(snapshot);
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
        } else if is_first_load
            && let Some(hint) = initial_selection_hint
            && visible.iter().any(|id| id == &hint)
        {
            // First-load launch-context hint: pre-select the row
            // the operator's cwd points at instead of the leading
            // tree row. Only honored on the first SetData so later
            // refreshes don't fight the operator's manual
            // selection.
            self.selection = Some(hint);
        } else {
            self.selection = Some(visible[0].clone());
        }
        self.recompute_detail();
    }

    fn move_selection(&mut self, delta: i32) {
        self.status_message = None;
        self.detail_links_expanded = false;
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
        self.status_message = None;
        self.detail_links_expanded = false;
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

    fn toggle_linked_details(&mut self) {
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

    fn recompute_detail(&mut self) {
        self.detail = None;
        self.preview_scroll = 0;
        let Some(selection) = self.selection.as_ref() else {
            self.explorer = None;
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
            self.explorer = None;
            return;
        };
        let Some(database) = self.database.as_ref() else {
            self.explorer = None;
            return;
        };
        let home = home_for_config(&self.config);
        self.detail = build_node_detail_from_conn(database.conn(), &target, home.as_deref())
            .expect("detail builder should read current TUI database");
        self.recompute_explorer_for(target, home.as_deref());
    }

    fn recompute_explorer_for(&mut self, target: NodeId, home: Option<&std::path::Path>) {
        let database = self.database.as_ref();
        let Some(database) = database else {
            self.explorer = None;
            return;
        };
        let view = build_node_view_from_conn(database.conn(), &target, home)
            .expect("explorer view builder should read current TUI database");
        match view {
            None => self.explorer = None,
            Some(view) => {
                // When the focused node hasn't changed, preserve
                // cursor / expansion / breadcrumb across refresh.
                let preserved =
                    self.explorer
                        .as_ref()
                        .and_then(|state| match state.view.focused == target {
                            true => Some(state.clone()),
                            false => None,
                        });
                match preserved {
                    Some(mut state) => {
                        let prev_key = state.selected_row().map(|row| row.key(&state.view));
                        // Replace the view while keeping cursor /
                        // expansion / breadcrumb identity.
                        state.view = view;
                        // Drop any expanded-group entries whose
                        // group no longer exists.
                        let valid: BTreeSet<GroupKey> = state
                            .view
                            .upstream
                            .groups
                            .iter()
                            .map(|g| {
                                GroupKey::for_group(crate::tui::explorer::Direction::Upstream, g)
                            })
                            .chain(state.view.downstream.groups.iter().map(|g| {
                                GroupKey::for_group(crate::tui::explorer::Direction::Downstream, g)
                            }))
                            .collect();
                        state.expanded_groups.retain(|key| valid.contains(key));
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

    fn explorer_move_cursor(&mut self, delta: i32) {
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let rows = state.rows();
        if rows.is_empty() {
            state.cursor = 0;
            return;
        }
        let len = rows.len() as i32;
        let next = (state.cursor as i32 + delta).clamp(0, len - 1);
        state.cursor = next as usize;
        self.status_message = None;
    }

    fn explorer_toggle_group(&mut self) {
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let rows = state.rows();
        let Some(row) = rows.get(state.cursor).cloned() else {
            return;
        };
        let ExplorerRow::GroupHeader {
            direction,
            group_index,
            ..
        } = row
        else {
            self.status_message = Some(
                "explorer: nothing to expand here — only multi-link groups expand".to_string(),
            );
            return;
        };
        let explorer = match direction {
            crate::tui::explorer::Direction::Upstream => &state.view.upstream,
            crate::tui::explorer::Direction::Downstream => &state.view.downstream,
        };
        let Some(group) = explorer.groups.get(group_index) else {
            return;
        };
        let key = GroupKey::for_group(direction, group);
        let prev_key = state.selected_row().map(|row| row.key(&state.view));
        if state.expanded_groups.contains(&key) {
            state.expanded_groups.remove(&key);
        } else {
            state.expanded_groups.insert(key);
        }
        state.reseat_cursor(prev_key);
        self.status_message = None;
    }

    fn explorer_activate(&mut self) {
        let Some(state) = self.explorer.as_ref() else {
            return;
        };
        let rows = state.rows();
        let Some(row) = rows.get(state.cursor).cloned() else {
            return;
        };
        match row {
            ExplorerRow::GroupHeader { .. } => {
                self.explorer_toggle_group();
            }
            ExplorerRow::Link { .. } => {
                if let Some(target) = state.view.drill_target(&row) {
                    self.explorer_drill_into(target);
                }
            }
            ExplorerRow::Unresolved { .. } => {
                self.status_message =
                    Some("explorer: unresolved evidence — `o` opens detail (T8-032)".to_string());
            }
            ExplorerRow::NodeField { .. } => {
                self.status_message = None;
            }
        }
    }

    fn explorer_drill_into(&mut self, target: NodeId) {
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let prev_focused = state.view.focused.clone();
        let prev_display = state.view.title_line.clone();
        let prev_cursor_key = state.selected_row().map(|row| row.key(&state.view));
        let prev_expanded = state.expanded_groups.clone();
        let hop = BreadcrumbHop {
            focused: prev_focused,
            display: prev_display,
            cursor_key: prev_cursor_key,
            expanded_groups: prev_expanded,
        };
        // Build the new view. If we can't load it, leave state alone
        // and surface a status message.
        let home = home_for_config(&self.config);
        let database = self.database.as_ref();
        let Some(database) = database else {
            return;
        };
        let next = build_node_view_from_conn(database.conn(), &target, home.as_deref())
            .expect("explorer view builder should read current TUI database");
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
                // transition (T8-029).
                self.detail =
                    build_node_detail_from_conn(database.conn(), &target, home.as_deref())
                        .expect("detail builder should read current TUI database");
                self.preview_scroll = 0;
                self.status_message = None;
            }
        }
    }

    fn explorer_back(&mut self) {
        let Some(state) = self.explorer.as_mut() else {
            return;
        };
        let Some(hop) = state.breadcrumb.pop() else {
            self.status_message = Some("explorer: no drill history to back out of".to_string());
            return;
        };
        let home = home_for_config(&self.config);
        let database = self.database.as_ref();
        let Some(database) = database else {
            return;
        };
        let view = build_node_view_from_conn(database.conn(), &hop.focused, home.as_deref())
            .expect("explorer view builder should read current TUI database");
        let Some(view) = view else {
            self.status_message = Some(
                "explorer: cannot restore breadcrumb hop — node missing from snapshot".to_string(),
            );
            return;
        };
        let breadcrumb_remaining = state.breadcrumb.clone();
        let mut restored = ExplorerState::new(view);
        restored.expanded_groups = hop.expanded_groups;
        restored.breadcrumb = breadcrumb_remaining;
        restored.reseat_cursor(hop.cursor_key);
        self.detail = build_node_detail_from_conn(database.conn(), &hop.focused, home.as_deref())
            .expect("detail builder should read current TUI database");
        self.explorer = Some(restored);
        self.preview_scroll = 0;
        self.status_message = None;
    }
}

fn initial_expanded_rows(tree: &RowTree) -> BTreeSet<RowId> {
    let mut expanded = BTreeSet::new();
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

fn add_expandable_group(expanded: &mut BTreeSet<RowId>, row: &Row) {
    if row.expandable && matches!(row.kind, RowKind::Group(_)) {
        expanded.insert(row.id.clone());
    }
}

fn home_for_config(_config: &RunConfig) -> Option<std::path::PathBuf> {
    // RunConfig doesn't carry the home directory today; the
    // dispatcher passes paths in already-shortened form via the
    // row tree, and the detail builder shortens its own when given
    // a home. For now we read the environment lazily - pure-state
    // tests can patch this when home-sensitive behavior matters.
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_scenarios;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, GraphNode, GraphSnapshot,
        RepoId, RepoNode,
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
            snap.nodes.push(GraphNode::Checkout(CheckoutNode {
                id: CheckoutId::new(repo_id, cwd.to_string()),
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
                last_active_epoch: None,
                session_kind: None,
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
            cwd: None,
            filter: RowFilter::default(),
        })
    }

    fn seeded_app(sessions: &[(&str, &str, &str)]) -> App {
        let snap = make_snapshot_with(sessions);
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app
    }

    fn scenario_app(name: &str) -> (App, GraphSnapshot) {
        let world = dev_scenarios::materialize(name).expect("materialize scenario");
        let snap = world.snapshot().expect("scenario snapshot");
        let tree = world.sessions_tree().expect("scenario sessions tree");
        let mut app = App::new(world.tui_config(View::Sessions, false));
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_600,
            initial_selection_hint: None,
        });
        (app, snap)
    }

    fn select_session(app: &mut App, session_key: &str) -> RowId {
        let id = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(session) if session.session.session_key == session_key => {
                    Some(row.id.clone())
                }
                _ => None,
            })
            .expect("visible session row");
        app.set_selection(id.clone());
        id
    }

    #[test]
    fn empty_tree_leaves_selection_none() {
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&GraphSnapshot::empty()),
            tree: RowTree::default(),
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        assert!(app.selection().is_none());
        assert!(app.detail().is_none());
    }

    #[test]
    fn set_data_default_expands_group_rows_and_selects_first_visible() {
        let app = seeded_app(&[("codex", "a", "/p/proj")]);
        let visible = app.visible_rows();
        assert!(
            !visible.is_empty(),
            "initial tree expands a visible starting context"
        );
        assert!(app.selection().is_some());
    }

    #[test]
    fn set_data_first_load_honors_initial_selection_hint() {
        // Build a tree with two project groups and feed the second
        // group's id as the launch-context hint on first SetData.
        // The reducer should pre-select the hinted row instead of
        // the leading row.
        let snap = make_snapshot_with(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
        let tree = build_tree(&snap);
        // Pick a group row whose id is *not* the first visible row.
        let first_group_id = tree
            .rows
            .iter()
            .find(|r| matches!(&r.kind, RowKind::Group(_)))
            .map(|r| r.id.clone())
            .expect("at least one group row");
        let hint = tree
            .rows
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::Group(_) if r.id != first_group_id => Some(r.id.clone()),
                _ => None,
            })
            .expect("at least two group rows in the seeded tree");

        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: Some(hint.clone()),
        });
        assert_eq!(app.selection().cloned(), Some(hint));
    }

    #[test]
    fn set_data_first_load_expands_only_launch_context_tree() {
        let snap = make_snapshot_with(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snap,
            grouping: SessionsGrouping::Graph,
            home: None,
            now: None,
            cwd: Some(std::path::Path::new("/p/projb")),
            filter: RowFilter::default(),
        });
        let hint = tree
            .rows
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::Group(group) if group.is_launch_context => Some(row.id.clone()),
                _ => None,
            })
            .expect("launch context row");

        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: Some(hint),
        });

        let visible = app.visible_rows();
        assert!(
            visible.iter().any(|row| {
                matches!(&row.kind, RowKind::AgentSession(s) if s.session.session_key == "b")
            }),
            "launch-context session should be visible"
        );
        assert!(
            !visible.iter().any(|row| {
                matches!(&row.kind, RowKind::AgentSession(s) if s.session.session_key == "a")
            }),
            "non-launch-context sessions should start collapsed"
        );
    }

    #[test]
    fn set_data_later_refreshes_ignore_initial_selection_hint() {
        // Seed the app once so prev_selection is populated, then
        // dispatch a second SetData with a hint that points
        // elsewhere. The retained selection should win.
        let mut app = seeded_app(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
        app.update(Msg::End); // move selection to the last row
        let kept = app.selection().cloned().expect("selection present");

        let snap = crate::query::read_snapshot(app.graph_db().unwrap().conn()).unwrap();
        let tree = build_tree(&snap);
        // Pick *some* other row id as the hint.
        let hint = tree
            .rows
            .iter()
            .map(|r| r.id.clone())
            .find(|id| id != &kept)
            .expect("at least one alternate row");
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_010,
            initial_selection_hint: Some(hint.clone()),
        });

        assert_eq!(
            app.selection().cloned(),
            Some(kept),
            "later refreshes must not overwrite the operator's selection with the hint"
        );
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
        app.update(Msg::ScrollPreviewBy(1));
        app.update(Msg::ScrollPreviewBy(1));
        assert_eq!(app.preview_scroll(), 2);
        app.update(Msg::ScrollPreviewBy(-1));
        app.update(Msg::ScrollPreviewBy(-1));
        app.update(Msg::ScrollPreviewBy(-1));
        assert_eq!(app.preview_scroll(), 0);
    }

    #[test]
    fn scroll_preview_by_advances_by_arbitrary_delta() {
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::ScrollPreviewBy(20));
        assert_eq!(app.preview_scroll(), 20);
        app.update(Msg::ScrollPreviewBy(-5));
        assert_eq!(app.preview_scroll(), 15);
        // Negative beyond zero clamps.
        app.update(Msg::ScrollPreviewBy(-1000));
        assert_eq!(app.preview_scroll(), 0);
    }

    #[test]
    fn adjust_left_scroll_keeps_selection_above_top() {
        let app = App::new(RunConfig::defaults());
        // Selection at line 0 with viewport 5: offset is 0.
        assert_eq!(app.adjust_left_scroll(0, 5), 0);
        // Walk the selection down to line 10; offset advances so
        // the selection is on the bottom edge of the viewport.
        assert_eq!(app.adjust_left_scroll(10, 5), 6);
        // Walk back up to line 4 — selection moved above the top
        // edge, so the offset retreats to it.
        assert_eq!(app.adjust_left_scroll(4, 5), 4);
    }

    #[test]
    fn adjust_left_scroll_holds_when_selection_inside_viewport() {
        let app = App::new(RunConfig::defaults());
        // Prime offset by scrolling to line 10 in a 5-tall viewport.
        app.adjust_left_scroll(10, 5);
        assert_eq!(app.left_scroll(), 6);
        // Move selection within [6, 10] — offset should not change.
        assert_eq!(app.adjust_left_scroll(8, 5), 6);
        assert_eq!(app.adjust_left_scroll(7, 5), 6);
        assert_eq!(app.adjust_left_scroll(10, 5), 6);
    }

    #[test]
    fn adjust_left_scroll_with_zero_viewport_does_nothing() {
        let app = App::new(RunConfig::defaults());
        app.adjust_left_scroll(10, 5);
        let before = app.left_scroll();
        let returned = app.adjust_left_scroll(99, 0);
        assert_eq!(returned, before);
        assert_eq!(app.left_scroll(), before);
    }

    #[test]
    fn scenario_ambiguous_mux_candidates_remain_navigable_after_expansion() {
        let (mut app, _) = scenario_app("ambiguous-mux");
        select_session(&mut app, "ambiguous");

        app.update(Msg::ToggleExpand);
        let candidate_rows: Vec<_> = app
            .visible_rows()
            .iter()
            .filter(|row| matches!(row.kind, RowKind::AgentSessionMuxCandidate(_)))
            .map(|row| row.id.clone())
            .collect();
        assert_eq!(
            candidate_rows.len(),
            2,
            "expanded ambiguous session should expose both mux candidates"
        );

        app.update(Msg::NavDown);
        assert_eq!(
            app.selection().cloned(),
            candidate_rows.first().cloned(),
            "cursor should move into candidate rows after expansion"
        );
        app.update(Msg::NavDown);
        assert_eq!(
            app.selection().cloned(),
            candidate_rows.get(1).cloned(),
            "cursor should move past the first candidate row"
        );
    }

    #[test]
    fn scenario_refresh_when_selected_row_disappears_snaps_to_visible_row() {
        let (mut app, _) = scenario_app("exact-match");
        let old_selection = select_session(&mut app, "session-x");

        let replacement =
            dev_scenarios::materialize("orphan-session").expect("materialize replacement scenario");
        let snap = replacement.snapshot().expect("replacement snapshot");
        let tree = replacement.sessions_tree().expect("replacement tree");
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_601,
            initial_selection_hint: None,
        });

        let new_selection = app.selection().cloned().expect("fallback selection");
        assert_ne!(
            new_selection, old_selection,
            "selected exact-match row should have disappeared"
        );
        assert!(
            app.visible_rows().iter().any(|row| row.id == new_selection),
            "fallback selection should point at a visible row"
        );
    }

    #[test]
    fn scenario_attach_target_refuses_current_tmux_session() {
        let world = dev_scenarios::materialize("exact-match").expect("materialize scenario");
        let snap = world.snapshot().expect("scenario snapshot");
        let tree = world.sessions_tree().expect("scenario sessions tree");
        let mut config = world.tui_config(View::Sessions, false);
        config.current_tmux_session = Some("editor".to_string());
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_600,
            initial_selection_hint: None,
        });
        select_session(&mut app, "session-x");

        assert_eq!(
            crate::tui::actions::resolve_attach_target(&app),
            Err(crate::tui::actions::AttachDisabled::CurrentTmuxSession(
                "editor".to_string()
            ))
        );
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
        let snap = crate::query::read_snapshot(app.graph_db().unwrap().conn()).unwrap();
        let tree = build_tree(&snap);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        assert_eq!(app.selection().cloned().unwrap(), saved);
    }

    #[test]
    fn set_data_falls_back_to_nearest_index_when_selection_disappears() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
        app.update(Msg::End);
        let original_selection = app.selection().cloned().unwrap();

        // Build a snapshot that drops the previously-selected row.
        let snap = make_snapshot_with(&[("codex", "a", "/p/proja")]);
        let tree = build_tree(&snap);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
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

    // ---- ADR 0031 / F8-003: per-view state retention ----

    #[test]
    fn switching_views_saves_active_state_and_loads_target_defaults() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        // Apply a sessions-only filter so we can observe it round-trip.
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetFilter(
            crate::filter::RowFilter {
                harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
                ..crate::filter::RowFilter::default()
            },
        ));
        // Switch to mux view; sessions state should park in the
        // per-view map and mux loads fresh defaults.
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SwitchView(
            View::Mux,
        ));
        assert_eq!(app.config().default_view, View::Mux);
        assert!(app.filter().is_empty(), "mux view starts unfiltered");
        assert_eq!(
            app.grouping(),
            crate::tui::Grouping::default_for(View::Mux),
            "mux view starts at its default grouping"
        );
        assert!(app.selection().is_none(), "fresh view has no selection");
    }

    #[test]
    fn switching_back_to_prior_view_restores_filter_and_grouping() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        let original_filter = crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
            ..crate::filter::RowFilter::default()
        };
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetFilter(
            original_filter.clone(),
        ));
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetGrouping(
            crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::Repo),
        ));
        // Switch away…
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SwitchView(
            View::Prs,
        ));
        // …and back.
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SwitchView(
            View::Sessions,
        ));
        assert_eq!(app.filter(), &original_filter);
        assert_eq!(
            app.grouping(),
            crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::Repo)
        );
    }

    #[test]
    fn switching_views_keeps_sort_global() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetSort(
            crate::tui::Sort::Recency,
        ));
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SwitchView(
            View::Mux,
        ));
        assert_eq!(app.sort(), crate::tui::Sort::Recency);
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SwitchView(
            View::Sessions,
        ));
        assert_eq!(app.sort(), crate::tui::Sort::Recency);
    }

    #[test]
    fn flat_sessions_grouping_forces_recency_sort() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetGrouping(
            crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::None),
        ));
        assert_eq!(app.sort(), crate::tui::Sort::Recency);

        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetSort(
            crate::tui::Sort::Hierarchy,
        ));
        assert_eq!(app.sort(), crate::tui::Sort::Recency);
    }

    #[test]
    fn returning_to_flat_sessions_grouping_restores_recency_sort() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetGrouping(
            crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::None),
        ));
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SwitchView(
            View::Mux,
        ));
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetSort(
            crate::tui::Sort::Hierarchy,
        ));
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SwitchView(
            View::Sessions,
        ));
        assert_eq!(app.sort(), crate::tui::Sort::Recency);
    }

    #[test]
    fn no_op_view_switch_is_idempotent() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetFilter(
            crate::filter::RowFilter {
                harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
                ..crate::filter::RowFilter::default()
            },
        ));
        let filter_before = app.filter().clone();
        let grouping_before = app.grouping();
        // SwitchView to the current view should be a no-op — not
        // a save+restore cycle that could wipe state.
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SwitchView(
            app.config().default_view,
        ));
        assert_eq!(app.filter(), &filter_before);
        assert_eq!(app.grouping(), grouping_before);
    }

    // ----- T8-028: explorer navigation / drilldown / breadcrumb -----

    use crate::model::{LinkEndpoint, LinkState, MuxSessionNode, RelationKind, SourceMetadata};
    use crate::tui::explorer::ExplorerRow;

    fn snapshot_session_with_mux() -> GraphSnapshot {
        // Session → linked_to_mux → mux. Drives the simplest
        // drillable explorer state: one downstream group with one
        // link.
        let mut snap = GraphSnapshot::empty();
        let repo_id = RepoId::new("/p/proj");
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
        snap.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(repo_id, "/p/proj".to_string()),
            root: "/p/proj".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("claude-code", "/state", "abc"),
            harness_key: "claude-code".to_string(),
            cwd: Some("/p/proj".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(1_700_000_000),
            session_kind: None,
        }));
        snap.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: crate::model::MuxSessionId::new("work"),
            backend: "tmux".to_string(),
            native_id: "work".to_string(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: Some(true),
            activity_epoch: Some(1_700_000_000),
            created_epoch: Some(1_700_000_000),
        }));
        let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
        let mux_id = NodeId::MuxSession(crate::model::MuxSessionId::new("work"));
        snap.candidate_links.push(crate::model::GraphLink {
            id: "l1".to_string(),
            source: session_id,
            target: LinkEndpoint::Node { id: mux_id },
            relation: RelationKind::LinkedToMux,
            provenance: crate::model::Provenance::StrongDiscovered,
            confidence: crate::model::Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        resolve_snapshot(snap)
    }

    fn app_for_explorer() -> App {
        let snap = snapshot_session_with_mux();
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        // Land selection on the agent session row.
        let row_id = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(_) => Some(row.id.clone()),
                _ => None,
            })
            .expect("agent session row in tree");
        app.set_selection(row_id);
        app
    }

    #[test]
    fn explorer_state_initializes_with_cursor_on_the_first_node_field() {
        // Defaulting to the first Node field keeps the Preview zone
        // showing the live mux capture / message preview the
        // operator expects to see by default. Pressing `j` walks
        // down into the relationship rows, at which point the
        // Preview switches to neighbor + edge content.
        let app = app_for_explorer();
        let state = app.explorer().expect("explorer state present");
        let row = state.selected_row().expect("selected row");
        assert!(matches!(row, ExplorerRow::NodeField { index: 0, .. }));
    }

    #[test]
    fn explorer_enter_on_link_drills_into_neighbor_and_pushes_breadcrumb() {
        let mut app = app_for_explorer();
        let before = app
            .explorer()
            .expect("explorer state present")
            .view
            .focused
            .clone();
        // Walk past the Node fields and onto the first link row,
        // then activate to drill.
        let target_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| matches!(row, ExplorerRow::Link { .. }))
            .expect("link row");
        for _ in 0..target_idx {
            app.update(Msg::ExplorerNavDown);
        }
        app.update(Msg::ExplorerActivate);
        let after = app.explorer().expect("explorer state after drill");
        // Focus now points at the mux.
        assert_ne!(after.view.focused, before);
        assert_eq!(after.view.kind_label, "mux_session");
        assert_eq!(after.breadcrumb.len(), 1);
        assert_eq!(after.breadcrumb[0].focused, before);
    }

    #[test]
    fn explorer_backspace_restores_previous_focused_node_and_cursor() {
        let mut app = app_for_explorer();
        // Walk past the Node fields and onto the first link row.
        let link_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| matches!(row, ExplorerRow::Link { .. }))
            .expect("link row");
        for _ in 0..link_idx {
            app.update(Msg::ExplorerNavDown);
        }
        let before_state = app.explorer().expect("initial").clone();
        let before_focus = before_state.view.focused.clone();
        let before_cursor_key = before_state
            .selected_row()
            .map(|row| row.key(&before_state.view));
        app.update(Msg::ExplorerActivate);
        // Move cursor on the new focused node to prove it gets
        // restored to the *original* one on Backspace.
        app.update(Msg::ExplorerNavDown);
        app.update(Msg::ExplorerBack);
        let restored = app.explorer().expect("restored explorer");
        assert_eq!(restored.view.focused, before_focus);
        assert!(restored.breadcrumb.is_empty());
        let restored_cursor = restored.selected_row().map(|row| row.key(&restored.view));
        assert_eq!(restored_cursor, before_cursor_key);
    }

    #[test]
    fn explorer_back_with_no_breadcrumb_surfaces_status_hint() {
        let mut app = app_for_explorer();
        app.update(Msg::ExplorerBack);
        assert!(
            app.status_message()
                .map(|s| s.contains("no drill history"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn explorer_toggle_group_only_acts_on_headers() {
        let mut app = app_for_explorer();
        // Walk past Node fields to the (single-link) composite row.
        let link_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| matches!(row, ExplorerRow::Link { .. }))
            .expect("link row");
        for _ in 0..link_idx {
            app.update(Msg::ExplorerNavDown);
        }
        app.update(Msg::ExplorerToggleGroup);
        assert!(
            app.status_message()
                .map(|s| s.contains("nothing to expand"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn explorer_nav_clamps_inside_flat_row_range() {
        let mut app = app_for_explorer();
        let len = app.explorer().expect("state").rows().len();
        for _ in 0..(len + 5) {
            app.update(Msg::ExplorerNavDown);
        }
        let cursor = app.explorer().expect("state").cursor;
        assert!(cursor < len.max(1));
        for _ in 0..(len + 5) {
            app.update(Msg::ExplorerNavUp);
        }
        assert_eq!(app.explorer().expect("state").cursor, 0);
    }

    #[test]
    fn open_value_modal_when_cursor_has_a_long_value() {
        // Build an app focused on a session that carries a long
        // `last_message_preview` so the cursor walks to a row with
        // a long_value set.
        let snap = {
            let mut snap = GraphSnapshot::empty();
            let repo_id = RepoId::new("/p/proj");
            snap.nodes
                .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
            snap.nodes.push(GraphNode::Checkout(CheckoutNode {
                id: CheckoutId::new(repo_id, "/p/proj".to_string()),
                root: "/p/proj".to_string(),
                git_dir: None,
                current_branch: None,
            }));
            snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("claude-code", "/state", "abc"),
                harness_key: "claude-code".to_string(),
                cwd: Some("/p/proj".to_string()),
                title: None,
                last_message_preview: Some("a".repeat(120)),
                last_active_epoch: Some(1_700_000_000),
                session_kind: None,
            }));
            resolve_snapshot(snap)
        };
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let row_id = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(_) => Some(row.id.clone()),
                _ => None,
            })
            .expect("session row");
        app.set_selection(row_id);
        // The session's `last_message_preview` lives only in
        // `all_fields`, not `core_fields`. Without the full-node
        // toggle (T8-034) the cursor never lands on it through
        // navigation. For now we exercise the no-value branch.
        app.open_value_modal_for_cursor();
        assert!(app.value_modal().is_none());
        assert!(
            app.status_message()
                .map(|s| s.contains("no truncated"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn value_modal_close_clears_state() {
        let mut app = app_for_explorer();
        // Simulate an opened modal — exercise the close path.
        app.value_modal = Some(crate::tui::widgets::value_modal::ValueModalState::new(
            "command",
            "long".to_string(),
        ));
        assert!(app.value_modal().is_some());
        app.close_value_modal();
        assert!(app.value_modal().is_none());
    }

    #[test]
    fn explorer_state_resets_when_left_tree_selection_changes() {
        let mut app = app_for_explorer();
        let initial_focused = app.explorer().expect("state").view.focused.clone();
        let link_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| matches!(row, ExplorerRow::Link { .. }))
            .expect("link row");
        for _ in 0..link_idx {
            app.update(Msg::ExplorerNavDown);
        }
        app.update(Msg::ExplorerActivate); // drill into mux
        let drilled_focused = app.explorer().expect("state").view.focused.clone();
        assert_ne!(initial_focused, drilled_focused);
        // Selecting a different row in the left tree should reset
        // the explorer to that new node — the right pane is the
        // detail surface for whatever the left pane points at.
        let snap = snapshot_session_with_mux();
        let other_session = AgentSessionNode {
            id: AgentSessionId::new("claude-code", "/state", "second"),
            harness_key: "claude-code".to_string(),
            cwd: Some("/p/proj".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(1_700_000_000),
            session_kind: None,
        };
        let mut snap = snap;
        snap.nodes.push(GraphNode::AgentSession(other_session));
        let snap = resolve_snapshot(snap);
        let tree = build_tree(&snap);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_100,
            initial_selection_hint: None,
        });
        // Re-select the original session — explorer follows the
        // selection.
        let row_id = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(session) if session.session.session_key == "abc" => {
                    Some(row.id.clone())
                }
                _ => None,
            })
            .expect("first session row");
        app.set_selection(row_id);
        let after = app.explorer().expect("explorer after reselect");
        assert_eq!(after.view.focused, initial_focused);
        assert!(after.breadcrumb.is_empty());
    }
}
