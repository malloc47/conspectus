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

use crate::model::{MuxSessionId, NodeId, PinId};
use crate::tui::detail::{DetailInputs, NodeDetail, build_node_detail};
use crate::tui::explorer::{
    BreadcrumbHop, ExplorerInputs, ExplorerRow, ExplorerRowKey, NodeView, build_node_view,
};
use crate::tui::preview::{PreviewContent, PreviewEntry, PreviewStore};
use crate::tui::rows::{Row, RowId, RowKind, RowTree};
use crate::tui::widgets::controls::HARNESS_OPTIONS;
use crate::tui::widgets::pins::{
    PinBindOption, PinCreateDefaults, PinCreateMode, PinMutationTarget,
};
use crate::tui::{RunConfig, View};

/// Reference-counted handle to the App's resolved
/// [`GraphSnapshot`]. Replaces the previous
/// `Rc<rusqlite::Connection>` wrapper (P11-011d) — the App now
/// holds the snapshot directly and every read consumer borrows
/// it via [`Self::snapshot`].
pub struct GraphDb(Rc<crate::model::GraphSnapshot>);

impl GraphDb {
    pub(crate) fn new(snapshot: crate::model::GraphSnapshot) -> Self {
        Self(Rc::new(snapshot))
    }

    #[cfg(test)]
    pub(crate) fn from_snapshot(snapshot: &crate::model::GraphSnapshot) -> Self {
        Self(Rc::new(snapshot.clone()))
    }

    pub(crate) fn snapshot(&self) -> &crate::model::GraphSnapshot {
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
        f.debug_tuple("GraphDb").field(&"<snapshot>").finish()
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
    /// Whether the explorer's link rows render the trailing
    /// `provenance · confidence · state` meta line (T8-042).
    /// Initialized from `RunConfig::show_edge_meta`; flipped at
    /// runtime by `Msg::ToggleEdgeMeta`.
    edge_meta_visible: bool,
    /// `true` after a Backspace press on the right pane with an
    /// empty breadcrumb stack: the press surfaced a hint instead of
    /// shifting focus, and a follow-up Backspace will perform the
    /// focus shift. Cleared by any other message so the arming only
    /// survives across consecutive Backspace presses.
    explorer_back_armed: bool,
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
    /// Right-panel explorer scroll offset, in rendered lines. Used
    /// to keep the explorer cursor visible inside the header section
    /// when the Related list grows past the section's height (the
    /// preview zone below reserves a minimum). Renderer-managed via
    /// [`App::adjust_explorer_scroll`] each frame, mirroring the
    /// left pane's pattern. Does not affect [`Self::preview_scroll`],
    /// which scrolls the preview body independently.
    explorer_scroll: Cell<u16>,
    /// Last visible-row index the selection landed on, used as a
    /// tiebreaker when the same `RowId` appears in multiple visible
    /// positions (e.g. the mux view lists the same ambiguously-
    /// attached session under every candidate mux). Without this,
    /// navigation looks up "current position" with `.position(...)`
    /// which always returns the first occurrence, and `j` from a
    /// later duplicate snaps the cursor back to the row after the
    /// first one.
    last_visible_index: Cell<Option<usize>>,
    /// Active rename overlay state per ADR 0029 / ADR 0030. `None`
    /// when no overlay is open; `Some` suspends the surrounding
    /// keymap and routes input through the modal.
    rename_overlay: Option<crate::tui::widgets::input::TextInputState>,
    /// Pin id waiting for a second `Delete` press. This gives pin
    /// removal a confirmation step without introducing a full modal
    /// before the H-PIN-023 edit/remove flow lands.
    pending_pin_remove: Option<String>,
    /// Active controls overlay (ADR 0031, F8-004). `None` when the
    /// overlay is closed; `Some` suspends the surrounding keymap
    /// and routes input through the modal.
    controls_overlay: Option<crate::tui::widgets::controls::ControlsOverlayState>,
    /// Active pins overlay (ADR 0057). Dedicated modal for pin CRUD,
    /// kept separate from the controls overlay so view/filter and
    /// pin management stay one-key-each on `f` and `p`.
    pins_overlay: Option<crate::tui::widgets::pins::PinsOverlayState>,
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
    /// Active full-screen transcript viewer modal
    /// (H-VIEWER-NATIVE-008). `None` when closed; `Some` replaces
    /// the entire two-panel layout with the native viewer widget
    /// and routes input through its reducer.
    viewer_modal: Option<crate::viewer::state::ViewerState>,
    /// Active transient toast (T8-040, H-WIDG-003). The
    /// `ratatui_comfy_toaster::ToastEngine` owns the per-toast
    /// lifetime + bordered rendering; the runtime calls
    /// [`Self::prepare_toast_for_render`] before each draw so
    /// `tick` retires expired entries and `set_area` follows the
    /// frame on resize. Wrapped in [`ToastEngineHolder`] because
    /// the upstream engine does not derive `Debug` — the holder
    /// satisfies the App-wide `#[derive(Debug)]` with a placeholder
    /// while transparently delegating via `Deref`/`DerefMut`.
    /// Non-blocking: input continues to flow to the underlying view.
    toast: crate::tui::widgets::toast::ToastEngineHolder,
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
    /// Optional persistence sink for the last-active view
    /// (F8-013). When `Some`, every view switch best-effort writes
    /// the new active view to
    /// `$XDG_STATE_HOME/conspectus/tui-state.json`. The runtime
    /// sets this at startup; `--no-resume-view` and `--snapshot`
    /// leave it `None` so the file stays untouched. Test apps
    /// likewise default to `None`.
    tui_state_cache: Option<crate::tui_state::TuiStateCache>,
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
    /// current `other_expanded` flag. Clamped on every update so the
    /// renderer can read it unchecked.
    pub cursor: usize,
    /// Whether the `Other` zone (alternates, conflicts, unresolved
    /// stubs) is currently expanded (ADR 0074 §3). Replaces the
    /// per-`GroupKey` expansion set from the prior layout.
    pub other_expanded: bool,
    /// Drill history. Empty when the focused node is the same one
    /// the left tree points at.
    pub breadcrumb: Vec<BreadcrumbHop>,
    /// Whether the Node zone is rendering its full per-kind field
    /// set (T8-034) instead of the top-5 Core summary. Per-focused-
    /// node: resets to `false` when drilling into a neighbor and is
    /// restored along with the prior focus by Backspace.
    pub full_detail_expanded: bool,
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
        Self {
            view,
            cursor: 0,
            other_expanded: false,
            breadcrumb: Vec::new(),
            full_detail_expanded: false,
        }
    }

    /// The flat list of selectable rows for the current view +
    /// expansion state. Computed each call rather than cached so the
    /// view model stays the source of truth.
    pub fn rows(&self) -> Vec<ExplorerRow> {
        self.view
            .flat_rows(self.other_expanded, self.full_detail_expanded)
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
            // Fall back to the first actionable row — a validated
            // link, then the Other header, then any Other link
            // (when expanded) — so the cursor never sits on the
            // node-field zone after a rebuild that previously had
            // it on a relationship row.
            rows.iter()
                .position(|row| {
                    matches!(
                        row,
                        ExplorerRow::ValidatedLink { .. }
                            | ExplorerRow::OtherHeader { .. }
                            | ExplorerRow::OtherLink { .. }
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
    /// Background data loader produced a new graph snapshot + row tree.
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
    /// Left panel: expand the selected row when it has children.
    /// No-op when the row is a leaf or is already expanded. Bound
    /// to `→` / `l` for vi-style tree navigation.
    ExpandRow,
    /// Left panel: collapse the selected row when it is expanded.
    /// No-op on already-collapsed or leaf rows. Bound to `←` / `h`.
    CollapseRow,
    /// Right panel: expand/collapse linked entity details under the
    /// selected node's compact link rows.
    ToggleLinkedDetails,
    /// Right panel (graph explorer): move the cursor down one row
    /// in the flat row list (T8-028).
    ExplorerNavDown,
    /// Right panel (graph explorer): move the cursor up one row.
    ExplorerNavUp,
    /// Right panel (graph explorer): snap the cursor to the first
    /// row in the flat list. Bound to `g` / `Home` on right-pane
    /// focus per H-OBS-007 so the operator gets the right-pane-
    /// equivalent behavior they get on the left tree.
    ExplorerHome,
    /// Right panel (graph explorer): snap the cursor to the last
    /// row in the flat list. Bound to `G` / `End` on right-pane
    /// focus per H-OBS-007.
    ExplorerEnd,
    /// Right panel (graph explorer): activate the highlighted row.
    /// On a group header this toggles the group's expansion; on a
    /// link row it drills into the neighbor and pushes a breadcrumb
    /// hop. No-op on unresolved-evidence rows in v1.
    ExplorerActivate,
    /// Right panel (graph explorer): toggle expansion of the
    /// highlighted multi-link group. No-op when the cursor isn't
    /// on a header.
    ExplorerToggleGroup,
    /// Right panel (graph explorer): toggle the visibility of the
    /// link rows' trailing `provenance · confidence · state` meta
    /// line (T8-042). The default is hidden; the `★` resolver-winner
    /// marker and `⚠` group-level conflict aggregate stay visible
    /// regardless.
    ToggleEdgeMeta,
    /// Right panel (graph explorer): toggle the Expanded Node Detail
    /// view (T8-034). Swaps the Node zone's top-5 render for the
    /// full per-kind field set. Per-focused-node: resets when
    /// drilling into a neighbor and is restored along with the
    /// prior focus on Backspace. No-op for node kinds whose
    /// `all_fields` matches `core_fields`.
    ExplorerToggleFullDetail,
    /// Right panel (graph explorer): back out of the most recent
    /// drilldown hop, restoring the previous focused node and the
    /// cursor / expansion state saved with it. When the breadcrumb
    /// stack is empty and the right pane has focus, shifts focus to
    /// the left pane so Backspace reads as a general "go back"
    /// gesture. No-op on left focus with an empty stack.
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
        let edge_meta_visible = config.show_edge_meta;
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
            edge_meta_visible,
            explorer_back_armed: false,
            preview_scroll: 0,
            loaded_at_epoch: None,
            status_message: None,
            provider_status: ProviderStatus::default(),
            refresh_failure: None,
            preview_store: PreviewStore::new(),
            left_scroll: Cell::new(0),
            explorer_scroll: Cell::new(0),
            last_visible_index: Cell::new(None),
            rename_overlay: None,
            pending_pin_remove: None,
            controls_overlay: None,
            pins_overlay: None,
            search_overlay: None,
            help_overlay: None,
            value_modal: None,
            viewer_modal: None,
            toast: crate::tui::widgets::toast::ToastEngineHolder(
                crate::tui::widgets::toast::engine(),
            ),
            sort,
            filter,
            grouping,
            view_states: BTreeMap::new(),
            tui_state_cache: None,
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
        // F8-013: persist the full UI state best-effort. When the
        // runtime hasn't enabled persistence (snapshot mode,
        // `--no-resume-view`, or any test path), the cache is `None`
        // and the call is a no-op. Failures here never abort the
        // switch.
        self.persist_state();
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

    pub fn pending_pin_remove(&self) -> Option<&str> {
        self.pending_pin_remove.as_deref()
    }

    pub fn set_pending_pin_remove(&mut self, pin_id: Option<String>) {
        self.pending_pin_remove = pin_id;
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

    /// Close the controls overlay without applying anything.
    pub fn close_controls_overlay(&mut self) {
        self.controls_overlay = None;
    }

    /// Active pins-overlay state (ADR 0057), if any.
    pub fn pins_overlay(&self) -> Option<&crate::tui::widgets::pins::PinsOverlayState> {
        self.pins_overlay.as_ref()
    }

    pub fn pins_overlay_mut(&mut self) -> Option<&mut crate::tui::widgets::pins::PinsOverlayState> {
        self.pins_overlay.as_mut()
    }

    /// Open the pins overlay at the top of the action list.
    pub fn open_pins_overlay(&mut self) {
        self.pins_overlay = Some(crate::tui::widgets::pins::PinsOverlayState::new());
    }

    /// Replace the pins overlay state — used by direct shortcuts
    /// (`N`/`B`/`A`/`b`) that skip the menu and open a sub-editor.
    pub fn set_pins_overlay(&mut self, state: crate::tui::widgets::pins::PinsOverlayState) {
        self.pins_overlay = Some(state);
    }

    /// Close the pins overlay without applying anything.
    pub fn close_pins_overlay(&mut self) {
        self.pins_overlay = None;
    }

    /// Snapshot of the live pin state the pins overlay renders against.
    pub fn pins_context(&self) -> crate::tui::widgets::pins::PinsContext {
        crate::tui::widgets::pins::PinsContext {
            pin_create_defaults: self.pin_create_defaults(),
            pin_adopt_defaults: self.pin_adopt_defaults_if_available(),
            known_harness_keys: self.known_harness_keys().into_iter().collect(),
            known_mux_names: self.used_mux_names().into_iter().collect(),
            known_pin_ids: self.used_pin_ids().into_iter().collect(),
            known_pin_mux_names: self.used_pin_mux_names().into_iter().collect(),
            selected_pin_id: self.selected_pin_id(),
            pin_target: self.pin_mutation_target(),
            pin_bind_options: self.pin_bind_options(),
        }
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

    /// Read-only access to the active transcript viewer modal
    /// (H-VIEWER-NATIVE-008). `None` when closed.
    pub fn viewer_modal(&self) -> Option<&crate::viewer::state::ViewerState> {
        self.viewer_modal.as_ref()
    }

    /// Take the viewer modal state out of the App so the pure
    /// reducer can consume it; callers put a new state back via
    /// [`Self::open_viewer_modal`] when the reducer returns
    /// `ViewerEffect::None`.
    pub fn take_viewer_modal(&mut self) -> Option<crate::viewer::state::ViewerState> {
        self.viewer_modal.take()
    }

    /// Mutable access for the draw path (the widget writes back
    /// viewport_height + total_lines metrics during render).
    pub fn viewer_modal_mut(&mut self) -> Option<&mut crate::viewer::state::ViewerState> {
        self.viewer_modal.as_mut()
    }

    pub fn open_viewer_modal(&mut self, state: crate::viewer::state::ViewerState) {
        self.viewer_modal = Some(state);
    }

    pub fn close_viewer_modal(&mut self) {
        self.viewer_modal = None;
    }

    /// Read accessor for the toast engine (T8-040 / H-WIDG-003).
    /// Returns the engine itself so the renderer can call
    /// `(&engine).render_ref(...)` directly; `has_toast()` reports
    /// whether anything is queued.
    pub fn toast(&self) -> &ratatui_comfy_toaster::ToastEngine<()> {
        &self.toast
    }

    /// Post a transient toast that auto-dismisses after the widget's
    /// `TOAST_DURATION` window. Called from the runtime side (the
    /// `Cmd` boundary per ADR 0024). Drains any prior queued toast
    /// first so the newer feedback supersedes — matches the in-tree
    /// "replacement" contract the reducer test pins.
    pub fn post_toast(&mut self, label: impl Into<String>) {
        let label = label.into();
        crate::tui::widgets::toast::engine_dismiss_all(&mut self.toast);
        self.toast
            .show_toast(crate::tui::widgets::toast::builder_for(label));
    }

    /// Update the engine's frame area and retire expired toasts.
    /// Called by the runtime once per draw — handles terminal
    /// resize and drives the polled expiry that replaces the prior
    /// in-tree `is_expired()` check.
    pub fn prepare_toast_for_render(&mut self, area: ratatui::layout::Rect) {
        self.toast.set_area(area);
        self.toast.tick();
    }

    /// Resolve whatever copyable value the explorer cursor points at.
    /// Returns `(label, value)` for the toast caption + clipboard
    /// payload, or `None` if the row is not a Node-zone field or the
    /// field has no value (empty or absent — see T8-040's
    /// "no misleading copied toast" contract).
    pub fn explorer_copy_target(&self) -> Option<(String, String)> {
        let state = self.explorer.as_ref()?;
        let row = state.selected_row()?;
        match row {
            ExplorerRow::NodeField { index, label, .. } => {
                let field = state.view.core_fields.get(index)?;
                // Prefer the untruncated form when present so the
                // clipboard always carries the full value; fall back
                // to the rendered short form for fields without a
                // separately-tracked long form.
                let value = field
                    .long_value
                    .clone()
                    .unwrap_or_else(|| field.value.clone());
                if value.is_empty() {
                    return None;
                }
                Some((label.to_string(), value))
            }
            _ => None,
        }
    }

    /// Resolve the full id of the selected agent or mux session row
    /// for the `i` keybinding. Returns `(label, id)` where `label` is
    /// the toast caption ("copied: agent_session id" /
    /// "copied: mux id") and `id` is the full id string (e.g.
    /// `agent_session:claude:proj_a:7d3f…`) the clipboard should
    /// receive. Returns `None` when the selection is anything other
    /// than an agent session or mux session row.
    pub fn selected_session_id(&self) -> Option<(String, String)> {
        match self.selection.as_ref()? {
            RowId::AgentSession(node @ NodeId::AgentSession(_)) => {
                Some(("copied: agent_session id".to_string(), node.to_string()))
            }
            RowId::MuxSession(node @ NodeId::MuxSession(_)) => {
                Some(("copied: mux id".to_string(), node.to_string()))
            }
            _ => None,
        }
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
        // Caller-driven jumps (search overlay commit, etc.) target a
        // specific RowId without a meaningful "current position", so
        // reset the duplicate-RowId tiebreaker. The next NavDown
        // falls back to the first occurrence in `visible_rows`, then
        // the cache repopulates from there.
        self.last_visible_index.set(None);
        self.selection = Some(id);
        self.status_message = None;
        self.recompute_detail();
    }

    /// After a successful pin create/adopt mutation and refresh,
    /// focus the row that represents `pin_id` in the current view.
    /// Grouped sessions/mux views prefer the synthetic Pins bucket;
    /// flat and union views fall back to the visible pinned entity
    /// row. Returns `false` when the refreshed view has no row for
    /// the pin (for example, an active filter hides it).
    pub fn select_pin_after_mutation(&mut self, pin_id: &str) -> bool {
        if self
            .tree
            .rows
            .iter()
            .any(|row| matches!(row.id, RowId::Synthetic("pins")) && row.expandable)
        {
            self.expanded.insert(RowId::Synthetic("pins"));
        }

        let visible = self.visible_rows();
        let pins_group_idx = visible
            .iter()
            .position(|row| matches!(row.id, RowId::Synthetic("pins")));
        let target = pins_group_idx
            .and_then(|idx| {
                let depth = visible[idx].depth;
                visible
                    .iter()
                    .enumerate()
                    .skip(idx + 1)
                    .take_while(|(_, row)| row.depth > depth)
                    .find_map(|(idx, row)| (row_pin_id(row) == Some(pin_id)).then_some(idx))
            })
            .or_else(|| {
                visible
                    .iter()
                    .enumerate()
                    .find_map(|(idx, row)| (row_pin_id(row) == Some(pin_id)).then_some(idx))
            });
        let Some(idx) = target else {
            return false;
        };
        let row_id = visible[idx].id.clone();
        self.last_visible_index.set(Some(idx));
        self.selection = Some(row_id);
        self.status_message = None;
        self.recompute_detail();
        true
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

    fn pin_bind_options(&self) -> Vec<PinBindOption> {
        crate::tui::actions::selected_pin_diagnostics(self)
            .into_iter()
            .find_map(|diagnostic| match diagnostic {
                crate::tui::actions::PinDiagnosticView::Ambiguous {
                    pin_id,
                    chosen,
                    competing,
                } => Some(
                    std::iter::once(chosen)
                        .chain(competing)
                        .map(|session| PinBindOption {
                            pin_id: pin_id.clone(),
                            session_key: session.session_key.clone(),
                            label: format!("{}:{}", session.harness_key, session.session_key),
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn pin_mutation_target(&self) -> Option<PinMutationTarget> {
        let selection = self.selection.as_ref()?;
        let row = self.tree.rows.iter().find(|row| &row.id == selection)?;
        let id = match &row.kind {
            RowKind::Pin(pin) => {
                return Some(PinMutationTarget {
                    id: pin.pin_id.clone(),
                    display_name: pin.display_name.clone(),
                    harness: pin.harness.clone(),
                    cwd: pin.cwd.clone(),
                    mux_name: pin.mux_name.clone(),
                    mux_socket: pin.mux_socket.clone(),
                    launch_argv: pin.launch_argv.clone(),
                    store_path: pin.store_path.clone(),
                });
            }
            RowKind::AgentSession(session) => session.pin_id.clone()?,
            _ => return None,
        };
        self.database.as_ref().and_then(|db| {
            db.snapshot()
                .pins
                .iter()
                .find(|pin| pin.id == id)
                .map(|pin| PinMutationTarget {
                    id: pin.id.clone(),
                    display_name: pin.display_name.clone(),
                    harness: pin.harness.clone(),
                    cwd: pin.cwd.clone(),
                    mux_name: pin.mux.name.clone(),
                    mux_socket: pin.mux.socket_name.clone(),
                    launch_argv: pin.launch_argv.clone().unwrap_or_default(),
                    store_path: pin.store_path.clone(),
                })
        })
    }

    fn pin_create_defaults(&self) -> PinCreateDefaults {
        let Some(selection) = self.selection.as_ref() else {
            return PinCreateDefaults::default();
        };
        let Some(row) = self.tree.rows.iter().find(|row| &row.id == selection) else {
            return PinCreateDefaults::default();
        };
        match &row.kind {
            RowKind::AgentSession(session) => {
                let cwd = self
                    .database
                    .as_ref()
                    .and_then(|db| {
                        db.snapshot().nodes.iter().find_map(|node| match node {
                            crate::model::GraphNode::AgentSession(node)
                                if node.id == session.session =>
                            {
                                node.cwd.clone()
                            }
                            _ => None,
                        })
                    })
                    .unwrap_or_default();
                let display = session
                    .display_label()
                    .map(str::to_string)
                    .unwrap_or_else(|| session.session.session_key.clone());
                let display = pin_create_default_name_candidate(&display);
                let id = pin_id_candidate(&display);
                let mux_name = self.unique_pin_mux_name(&id);
                PinCreateDefaults {
                    id: id.clone(),
                    display_name: display,
                    harness: session.session.harness_key.clone(),
                    cwd,
                    mux_name,
                    mode: PinCreateMode::NewVariation,
                }
            }
            RowKind::Group(group) => group
                .primary_node
                .as_ref()
                .and_then(pin_cwd_from_node)
                .map(|cwd| PinCreateDefaults {
                    cwd,
                    ..PinCreateDefaults::default()
                })
                .unwrap_or_default(),
            RowKind::MuxSession(mux) => {
                let base_name =
                    pin_create_default_name_candidate(&pin_id_candidate(&mux.native_id));
                let mux_name = self.unique_pin_mux_name(&base_name);
                PinCreateDefaults {
                    id: mux_name.clone(),
                    display_name: mux_name.clone(),
                    // Mirror the CLI `pin adopt` harness inference
                    // (`src/cli.rs:3883-3902`): the first active
                    // `LinkedToMux` candidate whose source is an
                    // AgentSession wins. Seeded as a default — the
                    // operator can still edit the field before commit.
                    harness: self.infer_harness_for_mux(&mux.mux).unwrap_or_default(),
                    // Read the raw cwd from the snapshot rather than
                    // `cwd_display`, which is tilde-shortened for
                    // rendering and would be rejected by the pin
                    // validator's `is_absolute` check on commit.
                    cwd: self.mux_cwd_for(&mux.mux).unwrap_or_default(),
                    mux_name,
                    mode: PinCreateMode::NewVariation,
                }
            }
            _ => PinCreateDefaults::default(),
        }
    }

    pub fn pin_adopt_defaults(&self) -> PinCreateDefaults {
        let mut defaults = self.pin_create_defaults();
        defaults.mode = PinCreateMode::AdoptSelected;
        if let Some(selection) = self.selection.as_ref()
            && let Some(row) = self.tree.rows.iter().find(|row| &row.id == selection)
            && let RowKind::MuxSession(mux) = &row.kind
        {
            defaults.id = pin_id_candidate(&mux.native_id);
            defaults.display_name = mux.native_id.clone();
            defaults.mux_name = mux.native_id.clone();
        }
        defaults
    }

    fn pin_adopt_defaults_if_available(&self) -> Option<PinCreateDefaults> {
        (self.selection_is_live_mux() && self.selected_pin_id().is_none())
            .then(|| self.pin_adopt_defaults())
    }

    fn used_pin_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        if let Some(database) = self.database.as_ref() {
            for pin in &database.snapshot().pins {
                ids.insert(pin.id.clone());
            }
        }
        ids
    }

    fn unique_pin_mux_name(&self, base: &str) -> String {
        let base = pin_id_candidate(base);
        let used = self.used_pin_mux_names();
        if !used.contains(&base) {
            return base;
        }
        for idx in 2.. {
            let candidate = format!("{base}-{idx}");
            if !used.contains(&candidate) {
                return candidate;
            }
        }
        unreachable!("unbounded suffix search must find a free mux name")
    }

    fn used_pin_mux_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        if let Some(database) = self.database.as_ref() {
            for node in &database.snapshot().nodes {
                if let crate::model::GraphNode::MuxSession(mux) = node {
                    names.insert(mux.native_id.clone());
                }
            }
            for pin in &database.snapshot().pins {
                names.insert(pin.mux.name.clone());
            }
        }
        names
    }

    fn used_mux_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        if let Some(database) = self.database.as_ref() {
            for node in &database.snapshot().nodes {
                if let crate::model::GraphNode::MuxSession(mux) = node {
                    names.insert(mux.native_id.clone());
                }
            }
        }
        names
    }

    fn selected_pin_id(&self) -> Option<String> {
        let selection = self.selection.as_ref()?;
        let row = self.tree.rows.iter().find(|row| &row.id == selection)?;
        row_pin_id(row).map(str::to_string)
    }

    fn known_harness_keys(&self) -> BTreeSet<String> {
        let mut keys: BTreeSet<String> =
            HARNESS_OPTIONS.iter().map(|key| key.to_string()).collect();
        if let Some(database) = self.database.as_ref() {
            for node in &database.snapshot().nodes {
                if let crate::model::GraphNode::AgentSession(session) = node
                    && !session.harness_key.trim().is_empty()
                {
                    keys.insert(session.harness_key.clone());
                }
            }
            for pin in &database.snapshot().pins {
                if !pin.harness.trim().is_empty() {
                    keys.insert(pin.harness.clone());
                }
            }
        }
        keys
    }

    /// Walk active `LinkedToMux` candidates whose target is `mux` and
    /// return the harness key of the first AgentSession source.
    /// Mirrors `PinAdoptArgs::run`'s inference path in `src/cli.rs`
    /// so the TUI `A` shortcut seeds the same harness the CLI's
    /// `pin adopt` would pick. Returns `None` when no active link
    /// attributes a harness to the mux.
    fn infer_harness_for_mux(&self, mux: &MuxSessionId) -> Option<String> {
        let db = self.database.as_ref()?;
        let snapshot = db.snapshot();
        let target = NodeId::MuxSession(mux.clone());
        snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == crate::model::RelationKind::LinkedToMux
                    && matches!(link.state, crate::model::LinkState::Active)
                    && link.target_node_id() == Some(&target)
            })
            .find_map(|link| match &link.source {
                NodeId::AgentSession(session) => Some(session.harness_key.clone()),
                _ => None,
            })
    }

    /// Look up the absolute `cwd` for a mux node from the persisted
    /// snapshot. Used to seed the pin-create form so the cwd field
    /// holds an absolute path that survives the validator in
    /// `pins::validate_entry`.
    fn mux_cwd_for(&self, mux: &MuxSessionId) -> Option<String> {
        let db = self.database.as_ref()?;
        db.snapshot().nodes.iter().find_map(|node| match node {
            crate::model::GraphNode::MuxSession(node) if &node.id == mux => node.cwd.clone(),
            _ => None,
        })
    }

    /// Returns true when the active selection is a live mux row.
    /// Used by the `A` (adopt) direct shortcut, which is scoped to
    /// mux rows because the rest of the flow seeds pin defaults from
    /// the mux's name / cwd.
    pub fn selection_is_live_mux(&self) -> bool {
        let Some(selection) = self.selection.as_ref() else {
            return false;
        };
        self.tree
            .rows
            .iter()
            .find(|row| &row.id == selection)
            .is_some_and(|row| matches!(row.kind, RowKind::MuxSession(_)))
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

    /// Apply a [`crate::tui::widgets::pins::PinsAction`] to the app
    /// state. Mirrors [`Self::apply_controls_action`] but covers the
    /// pin-specific surfaces (CRUD requests and the placeholder hint
    /// for menu entries without prerequisites). The CRUD write paths
    /// themselves live in the runtime so direct shortcuts can share
    /// the same plumbing.
    pub fn apply_pins_action(&mut self, action: crate::tui::widgets::pins::PinsAction) {
        use crate::tui::widgets::pins::PinsAction;
        match action {
            PinsAction::CreatePin(_) => {
                self.status_message =
                    Some("pins: create is handled by the TUI runtime".to_string());
            }
            PinsAction::EditPin(_) => {
                self.status_message = Some("pins: edit is handled by the TUI runtime".to_string());
            }
            PinsAction::BindPin(_) => {
                self.status_message = Some("pins: bind is handled by the TUI runtime".to_string());
            }
            PinsAction::RemovePin(_) => {
                self.status_message =
                    Some("pins: remove is handled by the TUI runtime".to_string());
            }
            PinsAction::LaunchPin { .. } => {
                self.status_message =
                    Some("pins: launch is handled by the TUI runtime".to_string());
            }
            PinsAction::PinPlaceholder(label) => {
                self.status_message = Some(format!(
                    "pins: `{label}` needs a pin selection; press `p` for the picker or use `conspectus pin {label}`"
                ));
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

    /// Test-only mutator for the run config so tests can twiddle
    /// fields like `current_tmux_session` after construction without
    /// rebuilding the entire fixture.
    #[cfg(test)]
    pub fn config_mut(&mut self) -> &mut RunConfig {
        &mut self.config
    }

    /// Enable F8-013 last-active-view persistence. The runtime calls
    /// this at startup with a `TuiStateCache` resolver pointed at
    /// `$XDG_STATE_HOME/conspectus/tui-state.json`. Snapshot mode
    /// and the `--no-resume-view` flag both leave the cache unset
    /// so view switches never touch the on-disk file.
    pub fn enable_view_persistence(&mut self, cache: crate::tui_state::TuiStateCache) {
        self.tui_state_cache = Some(cache);
    }

    /// Restore persisted sort, filter, and grouping from the state
    /// file. Called once at startup after [`Self::enable_view_persistence`].
    ///
    /// Fields explicitly set via CLI flags (`--sort`, `--grouping`,
    /// `--harness`, `--max-age`, `--mux-state`) are not overridden.
    pub fn restore_persisted_state(&mut self) {
        let Some(cache) = &self.tui_state_cache else {
            return;
        };
        let Some(persisted) = crate::tui_state::read_tui_state(cache) else {
            return;
        };

        // Global sort: win unless CLI explicitly set it.
        if !self.config.explicit_sort
            && let Some(sort) = persisted.sort
        {
            self.sort = sort;
            self.config.default_sort = sort;
        }

        // Pre-populate view_states from persisted state.
        for (view, slot) in persisted.view_states {
            let is_active = view == self.config.default_view;
            let filter = if is_active && self.config.explicit_filter {
                self.filter.clone()
            } else {
                slot.filter.clone()
            };
            let grouping = if is_active && self.config.explicit_grouping {
                self.grouping
            } else {
                slot.grouping
                    .unwrap_or_else(|| super::Grouping::default_for(view))
            };
            self.view_states
                .entry(view)
                .or_insert_with(|| ViewStateSlot {
                    filter,
                    grouping,
                    expanded: BTreeSet::new(),
                    selection: None,
                    left_scroll: 0,
                });

            // Apply to active fields for the current view.
            if is_active {
                if !self.config.explicit_filter {
                    self.filter = slot.filter.clone();
                    self.config.initial_filter = self.filter.clone();
                }
                if !self.config.explicit_grouping
                    && let Some(g) = slot.grouping
                {
                    self.grouping = g;
                    self.apply_grouping_to_config(g);
                }
            }
        }
        self.force_recency_for_flat_sessions();
    }

    /// Build a [`PersistedState`] snapshot of the current app state
    /// for writing to the state file. Captures the last-active view,
    /// global sort, and per-view filter/grouping from `view_states`
    /// (plus the active view's current state, which may not yet be
    /// in `view_states`).
    pub fn build_persisted_state(&self) -> crate::tui_state::PersistedState {
        let mut view_states: BTreeMap<View, crate::tui_state::PersistedViewSlot> = BTreeMap::new();
        let active_view = self.config.default_view;
        // Add the active view's current state.
        view_states.insert(
            active_view,
            crate::tui_state::PersistedViewSlot {
                filter: self.filter.clone(),
                grouping: Some(self.grouping),
            },
        );
        // Merge other views from saved slots (skip the active one).
        for (&view, slot) in &self.view_states {
            if view != active_view {
                view_states
                    .entry(view)
                    .or_insert_with(|| crate::tui_state::PersistedViewSlot {
                        filter: slot.filter.clone(),
                        grouping: Some(slot.grouping),
                    });
            }
        }
        crate::tui_state::PersistedState {
            last_view: Some(self.config.default_view),
            sort: Some(self.sort),
            view_states,
        }
    }

    /// Write the current TUI state to disk. Best-effort; failures are
    /// silently swallowed.
    pub(crate) fn persist_state(&self) {
        if let Some(cache) = &self.tui_state_cache {
            let state = self.build_persisted_state();
            let _ = crate::tui_state::write_tui_state(cache, &state);
        }
    }

    fn apply_grouping_to_config(&mut self, grouping: super::Grouping) {
        match grouping {
            super::Grouping::Sessions(g) => {
                self.config.sessions_grouping = g;
            }
            super::Grouping::Mux(g) => {
                self.config.mux_grouping = g;
            }
            super::Grouping::Union(_) | super::Grouping::Prs(_) | super::Grouping::Forks(_) => {}
        }
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

    /// Whether the explorer's link rows render the trailing
    /// `provenance · confidence · state` meta line (T8-042).
    pub fn edge_meta_visible(&self) -> bool {
        self.edge_meta_visible
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

    /// Reconcile the explorer (right-pane) scroll offset against the
    /// cursor's rendered (post-wrap) row span and the header
    /// viewport height, returning the new offset to apply to
    /// `Paragraph::scroll`. Mirrors [`Self::adjust_left_scroll`].
    ///
    /// `cursor_first_row` and `cursor_last_row` bracket the rendered
    /// rows the cursor's logical line occupies. Equal values describe
    /// a single-row line; when the line wraps, `cursor_last_row` is
    /// the row index of its final terminal row. The right pane uses
    /// `Paragraph::wrap`, so a long value (path, native_id) routinely
    /// spans 2+ rows; without the span the offset only guaranteed
    /// the cursor's *start* row was visible and the trailing wrapped
    /// rows fell off the bottom by one.
    ///
    /// Triggered every frame so an out-of-date offset self-corrects
    /// without explicit invalidation on focus changes or rebuilds.
    pub fn adjust_explorer_scroll(
        &self,
        cursor_first_row: usize,
        cursor_last_row: usize,
        viewport_height: u16,
    ) -> u16 {
        let vh = viewport_height as usize;
        if vh == 0 {
            return self.explorer_scroll.get();
        }
        let mut offset = self.explorer_scroll.get() as usize;
        // Scroll up if the cursor's first row is above the top of
        // the viewport.
        if cursor_first_row < offset {
            offset = cursor_first_row;
        }
        // Scroll down if the cursor's last row is at or past the
        // bottom of the viewport. Using the cursor span's last row
        // (not its first) keeps wrapped cursor lines fully visible.
        if cursor_last_row >= offset + vh {
            offset = cursor_last_row + 1 - vh;
        }
        let clamped = offset.min(u16::MAX as usize) as u16;
        self.explorer_scroll.set(clamped);
        clamped
    }

    /// Test-only accessor for the current explorer scroll offset.
    #[cfg(test)]
    pub fn explorer_scroll(&self) -> u16 {
        self.explorer_scroll.get()
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
        // The empty-stack Backspace arming only persists across
        // consecutive Backspace presses; any other message clears it
        // so the operator doesn't accidentally back out of the right
        // pane after an intervening action.
        if !matches!(msg, Msg::ExplorerBack) {
            self.explorer_back_armed = false;
        }
        match msg {
            Msg::Quit => self.should_quit = true,
            Msg::SetData {
                snapshot,
                tree,
                loaded_at_epoch,
                initial_selection_hint,
            } => {
                self.pending_pin_remove = None;
                self.set_data(snapshot, tree, loaded_at_epoch, initial_selection_hint);
            }
            Msg::NavDown => {
                self.pending_pin_remove = None;
                self.move_selection(1);
            }
            Msg::NavUp => {
                self.pending_pin_remove = None;
                self.move_selection(-1);
            }
            Msg::PageDown(viewport) => {
                self.pending_pin_remove = None;
                self.move_selection(i32::from(viewport.max(1)));
            }
            Msg::PageUp(viewport) => {
                self.pending_pin_remove = None;
                self.move_selection(-i32::from(viewport.max(1)));
            }
            Msg::Home => {
                self.pending_pin_remove = None;
                self.move_selection_to(0);
            }
            Msg::End => {
                self.pending_pin_remove = None;
                self.move_selection_to(usize::MAX);
            }
            Msg::ToggleExpand => self.toggle_expand_selected(),
            Msg::ExpandRow => self.expand_selected(),
            Msg::CollapseRow => self.collapse_selected(),
            Msg::ToggleLinkedDetails => self.toggle_linked_details(),
            Msg::ExplorerNavDown => self.explorer_move_cursor(1),
            Msg::ExplorerNavUp => self.explorer_move_cursor(-1),
            Msg::ExplorerHome => self.explorer_jump_cursor_to(0),
            Msg::ExplorerEnd => self.explorer_jump_cursor_to(usize::MAX),
            Msg::ExplorerActivate => self.explorer_activate(),
            Msg::ExplorerToggleGroup => self.explorer_toggle_group(),
            Msg::ExplorerToggleFullDetail => self.explorer_toggle_full_detail(),
            Msg::ToggleEdgeMeta => self.toggle_edge_meta(),
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
            self.last_visible_index.set(None);
        } else if let Some(prev) = prev_selection.as_ref()
            && let Some(pos) = self.position_closest_to(&visible, prev, prev_visible_index)
        {
            self.selection = Some(visible[pos].clone());
            self.last_visible_index.set(Some(pos));
        } else if let Some(prev_index) = prev_visible_index {
            let clamped = prev_index.min(visible.len() - 1);
            self.selection = Some(visible[clamped].clone());
            self.last_visible_index.set(Some(clamped));
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
            self.last_visible_index.set(Some(pos));
        } else {
            self.selection = Some(visible[0].clone());
            self.last_visible_index.set(Some(0));
        }
        self.recompute_detail();
    }

    /// Find `selection` in `visible`, preferring the occurrence
    /// closest to `hint` when the id appears more than once.
    /// Returns `None` when the id is gone (the refresh dropped the
    /// row). Used by [`Self::set_data`] so refreshes don't snap a
    /// stable cursor to the first copy of a duplicated row.
    fn position_closest_to(
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

    fn move_selection(&mut self, delta: i32) {
        self.status_message = None;
        self.detail_links_expanded = false;
        let visible = self.visible_rows_owned();
        if visible.is_empty() {
            self.selection = None;
            self.detail = None;
            self.last_visible_index.set(None);
            return;
        }
        let current = self
            .selection
            .as_ref()
            .map(|id| self.current_visible_index(&visible, id))
            .unwrap_or(0);
        let len = visible.len() as i32;
        let target = (current as i32 + delta).clamp(0, len - 1) as usize;
        self.selection = Some(visible[target].clone());
        self.last_visible_index.set(Some(target));
        self.recompute_detail();
    }

    fn move_selection_to(&mut self, index: usize) {
        self.status_message = None;
        self.detail_links_expanded = false;
        let visible = self.visible_rows_owned();
        if visible.is_empty() {
            self.selection = None;
            self.detail = None;
            self.last_visible_index.set(None);
            return;
        }
        let clamped = index.min(visible.len() - 1);
        self.selection = Some(visible[clamped].clone());
        self.last_visible_index.set(Some(clamped));
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
    fn current_visible_index(&self, visible: &[RowId], selection: &RowId) -> usize {
        let mut matches = visible
            .iter()
            .enumerate()
            .filter_map(|(idx, id)| (id == selection).then_some(idx));
        let Some(first) = matches.next() else {
            return 0;
        };
        let Some(cached) = self.last_visible_index.get() else {
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

    fn expand_selected(&mut self) {
        let Some(id) = self.selection.as_ref().cloned() else {
            return;
        };
        let is_expandable = self.tree.rows.iter().any(|r| r.id == id && r.expandable);
        if !is_expandable {
            return;
        }
        self.expanded.insert(id);
    }

    fn collapse_selected(&mut self) {
        let Some(id) = self.selection.as_ref().cloned() else {
            return;
        };
        let is_expandable = self.tree.rows.iter().any(|r| r.id == id && r.expandable);
        if !is_expandable {
            return;
        }
        self.expanded.remove(&id);
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
            RowId::Pin { pin_id } => Some(NodeId::Pin(PinId::new(pin_id.clone()))),
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
        self.detail = build_node_detail(DetailInputs {
            snapshot: database.snapshot(),
            target: &target,
            home: home.as_deref(),
        });
        self.recompute_explorer_for(target, home.as_deref());
    }

    fn recompute_explorer_for(&mut self, target: NodeId, home: Option<&std::path::Path>) {
        let database = self.database.as_ref();
        let Some(database) = database else {
            self.explorer = None;
            return;
        };
        let view = build_node_view(ExplorerInputs {
            snapshot: database.snapshot(),
            target: &target,
            home,
        });
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

    /// Snap the explorer cursor to `index`, clamped to the current
    /// row count. `usize::MAX` is the convention for "last row" so
    /// callers can ask for End without needing to recompute row
    /// counts themselves; this mirrors how `move_selection_to` on
    /// the left-pane tree handles `Msg::End` (H-OBS-007).
    fn explorer_jump_cursor_to(&mut self, index: usize) {
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

    fn explorer_toggle_group(&mut self) {
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

    fn toggle_edge_meta(&mut self) {
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

    fn explorer_toggle_full_detail(&mut self) {
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

    fn explorer_activate(&mut self) {
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

    fn explorer_drill_into(&mut self, target: NodeId) {
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
        let database = self.database.as_ref();
        let Some(database) = database else {
            return;
        };
        let next = build_node_view(ExplorerInputs {
            snapshot: database.snapshot(),
            target: &target,
            home: home.as_deref(),
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
                // transition (T8-029).
                self.detail = build_node_detail(DetailInputs {
                    snapshot: database.snapshot(),
                    target: &target,
                    home: home.as_deref(),
                });
                self.preview_scroll = 0;
                self.status_message = None;
                // T8-035: mirror sync — when the drilled neighbor
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
    /// view's row tree (T8-035 fallback per the design doc).
    fn mirror_left_pane_to(&mut self, target: &NodeId) {
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

    fn explorer_back(&mut self) {
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
        let database = self.database.as_ref();
        let Some(database) = database else {
            return;
        };
        let view = build_node_view(ExplorerInputs {
            snapshot: database.snapshot(),
            target: &hop.focused,
            home: home.as_deref(),
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
            snapshot: database.snapshot(),
            target: &hop.focused,
            home: home.as_deref(),
        });
        self.explorer = Some(restored);
        self.preview_scroll = 0;
        self.status_message = None;
        // T8-035: restore the left-pane selection that was active
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

fn pin_cwd_from_node(id: &NodeId) -> Option<String> {
    match id {
        NodeId::Checkout(checkout) => Some(checkout.root.clone()),
        NodeId::Repo(repo) => repo
            .common_dir
            .strip_suffix("/.git")
            .map(str::to_string)
            .or_else(|| Some(repo.common_dir.clone())),
        _ => None,
    }
}

fn pin_id_candidate(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "new-pin".to_string()
    } else {
        trimmed
    }
}

const PIN_CREATE_DEFAULT_NAME_MAX_CHARS: usize = 48;

fn pin_create_default_name_candidate(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= PIN_CREATE_DEFAULT_NAME_MAX_CHARS {
        return trimmed.to_string();
    }
    let capped: String = trimmed
        .chars()
        .take(PIN_CREATE_DEFAULT_NAME_MAX_CHARS)
        .collect();
    let capped = if trimmed
        .chars()
        .nth(PIN_CREATE_DEFAULT_NAME_MAX_CHARS)
        .is_some_and(|ch| !ch.is_whitespace())
    {
        capped
            .rfind(char::is_whitespace)
            .map(|idx| capped[..idx].to_string())
            .unwrap_or(capped)
    } else {
        capped
    };
    let capped = capped
        .trim_end_matches(|ch: char| ch.is_whitespace() || ch == '-' || ch == '_')
        .to_string();
    if capped.is_empty() {
        "new pin".to_string()
    } else {
        capped
    }
}

/// Does `row` represent `target` in the left-pane tree (T8-035)?
/// Group rows match when their `primary_node` (when set) equals
/// `target`; mux candidate rows match their parent mux's node id.
fn row_matches(row: &crate::tui::rows::Row, target: &NodeId) -> bool {
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

fn row_pin_id(row: &crate::tui::rows::Row) -> Option<&str> {
    match &row.kind {
        RowKind::AgentSession(session) => session.pin_id.as_deref(),
        RowKind::MuxSession(mux) => mux.pin_id.as_deref(),
        RowKind::Pin(pin) => Some(pin.pin_id.as_str()),
        _ => None,
    }
}

fn initial_expanded_rows(tree: &RowTree) -> BTreeSet<RowId> {
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
        PinBinding, PinCandidate, PinMuxRef, Provenance, RepoId, RepoNode, WorkspaceId,
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
    fn pins_context_seeds_pin_create_defaults_from_selected_session() {
        let mut app = seeded_app(&[("codex", "Session One", "/p/project")]);
        select_session(&mut app, "Session One");

        let defaults = app.pins_context().pin_create_defaults;
        assert_eq!(defaults.id, "session-one");
        assert_eq!(defaults.display_name, "Session One");
        assert_eq!(defaults.harness, "codex");
        assert_eq!(defaults.cwd, "/p/project");
        assert_eq!(defaults.mux_name, "session-one");
    }

    #[test]
    fn pin_create_defaults_cap_long_selected_session_titles() {
        let long_title =
            "Investigate the customer workspace regression with the unusually verbose summary";
        let mut app = seeded_app(&[("codex", long_title, "/p/project")]);
        select_session(&mut app, long_title);

        let defaults = app.pins_context().pin_create_defaults;
        assert!(defaults.display_name.chars().count() <= 48);
        assert_eq!(
            defaults.display_name,
            "Investigate the customer workspace regression"
        );
        assert_eq!(defaults.id, "investigate-the-customer-workspace-regression");
        assert_eq!(defaults.mux_name, defaults.id);
    }

    #[test]
    fn pins_context_seeds_pin_create_cwd_from_selected_group() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        let group_id = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::Group(group) if group.primary_node.is_some() => Some(row.id.clone()),
                _ => None,
            })
            .expect("group row with primary node");
        app.set_selection(group_id);

        let defaults = app.pins_context().pin_create_defaults;
        assert_eq!(defaults.cwd, "/p/proja");
        assert_eq!(defaults.id, "");
        assert_eq!(defaults.harness, "");
    }

    #[test]
    fn pins_context_seeds_pin_create_cwd_from_selected_mux_absolute() {
        // Regression: the MuxSession branch of `pin_create_defaults`
        // used to seed `cwd` from `MuxSessionRow.cwd_display`, which
        // tilde-shortens the path. The pin write path then rejected
        // it via `Path::is_absolute`. The form must instead carry
        // the raw absolute cwd off the mux node.
        let mut snap = snapshot_session_with_mux();
        for node in snap.nodes.iter_mut() {
            if let crate::model::GraphNode::MuxSession(mux) = node {
                mux.cwd = Some("/p/proj".to_string());
            }
        }
        let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
            snapshot: &snap,
            home: None,
            now: None,
            filter: crate::tui::RowFilter::default(),
            grouping: crate::tui::MuxGrouping::Session,
            sort: crate::tui::Sort::Hierarchy,
        });
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let mux_row_id = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::MuxSession(_) => Some(row.id.clone()),
                _ => None,
            })
            .expect("mux row in tree");
        app.set_selection(mux_row_id);

        let defaults = app.pins_context().pin_create_defaults;
        assert_eq!(defaults.cwd, "/p/proj");
        assert_eq!(defaults.mode, PinCreateMode::NewVariation);
        assert_eq!(defaults.mux_name, "work-2");
        assert_eq!(defaults.display_name, "work-2");
        assert!(
            std::path::Path::new(&defaults.cwd).is_absolute(),
            "pin create cwd must be absolute: {:?}",
            defaults.cwd,
        );
    }

    #[test]
    fn pin_adopt_defaults_preserve_selected_live_mux_name() {
        let snap = snapshot_session_with_mux();
        let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
            snapshot: &snap,
            home: None,
            now: None,
            filter: crate::tui::RowFilter::default(),
            grouping: crate::tui::MuxGrouping::Session,
            sort: crate::tui::Sort::Hierarchy,
        });
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let mux_row_id = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::MuxSession(_) => Some(row.id.clone()),
                _ => None,
            })
            .expect("mux row in tree");
        app.set_selection(mux_row_id);

        let defaults = app.pin_adopt_defaults();
        assert_eq!(defaults.mode, PinCreateMode::AdoptSelected);
        assert_eq!(defaults.id, "work");
        assert_eq!(defaults.display_name, "work");
        assert_eq!(defaults.mux_name, "work");
    }

    #[test]
    fn pins_context_exposes_known_live_mux_names() {
        let snap = snapshot_session_with_mux();
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });

        let ctx = app.pins_context();
        assert_eq!(ctx.known_mux_names, vec!["work".to_string()]);
    }

    #[test]
    fn pins_context_does_not_offer_adopt_for_already_pinned_mux() {
        let mut snap = snapshot_session_with_mux();
        snap.pins.push(PinCandidate {
            id: "work-pin".to_string(),
            display_name: "Work".to_string(),
            harness: "claude-code".to_string(),
            cwd: "/p/proj".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "work".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/proj/.conspectus.toml".to_string(),
            binding: Some(PinBinding::StaleMux {
                mux: crate::model::MuxSessionId::new("work"),
            }),
        });
        let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
            snapshot: &snap,
            home: None,
            now: None,
            filter: crate::tui::RowFilter::default(),
            grouping: crate::tui::MuxGrouping::Repo,
            sort: crate::tui::Sort::Hierarchy,
        });
        let mut app = App::new(RunConfig {
            default_view: View::Mux,
            mux_grouping: crate::tui::MuxGrouping::Repo,
            ..RunConfig::defaults()
        });
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let mux_row_id = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::MuxSession(mux) if mux.native_id == "work" => Some(row.id.clone()),
                _ => None,
            })
            .expect("pinned mux row");
        app.set_selection(mux_row_id);

        let ctx = app.pins_context();
        assert_eq!(ctx.selected_pin_id.as_deref(), Some("work-pin"));
        assert_eq!(ctx.known_pin_ids, vec!["work-pin".to_string()]);
        assert_eq!(ctx.known_pin_mux_names, vec!["work".to_string()]);
        assert_eq!(ctx.pin_adopt_defaults, None);
    }

    #[test]
    fn pins_context_exposes_registered_and_discovered_harness_keys() {
        let snap = make_snapshot_with(&[("custom-harness", "s1", "/workspace/project")]);
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });

        let ctx = app.pins_context();
        assert!(ctx.known_harness_keys.contains(&"codex".to_string()));
        assert!(
            ctx.known_harness_keys
                .contains(&"custom-harness".to_string())
        );
    }

    #[test]
    fn infer_harness_for_mux_picks_first_active_linked_to_mux_source() {
        // Reuses the session+mux+LinkedToMux fixture defined below in
        // the explorer test block: one claude-code AgentSession linked
        // to a `tmux:work` MuxSession via an Active LinkedToMux
        // candidate. The inference walk should land on
        // `claude-code` as the harness seed.
        let snap = snapshot_session_with_mux();
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let mux_id = crate::model::MuxSessionId::new("work");
        assert_eq!(
            app.infer_harness_for_mux(&mux_id).as_deref(),
            Some("claude-code"),
        );
    }

    #[test]
    fn infer_harness_for_mux_returns_none_without_attribution() {
        // Empty graph → no candidate links → no inference. Confirms the
        // method is safe to call from `pin_create_defaults` regardless
        // of selection state.
        let app = App::new(RunConfig::defaults());
        let mux_id = crate::model::MuxSessionId::new("nowhere");
        assert!(app.infer_harness_for_mux(&mux_id).is_none());
    }

    #[test]
    fn pins_context_seeds_pin_mutation_target_from_selected_pin_row() {
        let mut snap = GraphSnapshot::empty();
        snap.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/project".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/project/.conspectus.toml".to_string(),
            binding: None,
        });
        let snap = resolve_snapshot(snap);
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let pin_id = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::Pin(_) => Some(row.id.clone()),
                _ => None,
            })
            .expect("pin row");
        app.set_selection(pin_id);

        let target = app.pins_context().pin_target.expect("pin mutation target");
        assert_eq!(target.id, "ingest");
        assert_eq!(target.display_name, "Ingest");
        assert_eq!(target.harness, "codex");
        assert_eq!(target.cwd, "/p/project");
        assert_eq!(target.mux_name, "ingest");
        assert_eq!(target.mux_socket, None);
        assert_eq!(target.launch_argv, Vec::<String>::new());
        assert_eq!(target.store_path, "/p/project/.conspectus.toml");
    }

    #[test]
    fn set_data_keeps_pins_group_expanded_by_default() {
        let mut snap = GraphSnapshot::empty();
        snap.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/project".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/project/.conspectus.toml".to_string(),
            binding: None,
        });
        let snap = resolve_snapshot(snap);
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });

        assert!(app.expanded.contains(&RowId::Synthetic("pins")));
        assert!(
            app.visible_rows()
                .iter()
                .any(|row| matches!(row.id, RowId::Pin { .. })),
            "pin child should be visible without manually expanding Pins"
        );
    }

    #[test]
    fn selected_bound_pin_row_shows_realizing_session_detail() {
        let session_id = AgentSessionId::new("codex", "/state", "alpha");
        let mut snap = make_snapshot_with(&[("codex", "alpha", "/p/project")]);
        snap.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/project".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/project/.conspectus.toml".to_string(),
            binding: Some(crate::model::PinBinding::Bound {
                mux: crate::model::MuxSessionId::new("tmux:ingest"),
                session: session_id,
            }),
        });
        snap.sync_pin_nodes();
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
                RowKind::AgentSession(session) if session.pin_id.as_deref() == Some("ingest") => {
                    Some(row.id.clone())
                }
                _ => None,
            })
            .expect("bound pin should render as a session row");
        app.set_selection(row_id);

        let detail = app
            .detail()
            .expect("bound pin session row should resolve detail");
        assert_eq!(detail.kind_label, "agent_session");
        assert!(
            detail
                .header_fields
                .iter()
                .any(|field| { field.label == "harness" && field.value.contains("codex") })
        );
    }

    #[test]
    fn select_pin_after_mutation_expands_pins_and_selects_session_row() {
        let session_id = AgentSessionId::new("codex", "/state", "alpha");
        let mut snap = make_snapshot_with(&[("codex", "alpha", "/p/project")]);
        snap.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/project".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/project/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Bound {
                mux: crate::model::MuxSessionId::new("tmux:ingest"),
                session: session_id.clone(),
            }),
        });
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app.expanded.remove(&RowId::Synthetic("pins"));

        assert!(app.select_pin_after_mutation("ingest"));
        assert!(app.expanded.contains(&RowId::Synthetic("pins")));
        let selected = app.selection().expect("selection");
        assert!(
            matches!(selected, RowId::AgentSession(NodeId::AgentSession(id)) if id == &session_id)
        );
        let first_visible_pin_row = app
            .visible_rows()
            .into_iter()
            .find(|row| row_pin_id(row) == Some("ingest"))
            .expect("visible pinned row");
        assert_eq!(&first_visible_pin_row.id, selected);
    }

    #[test]
    fn select_pin_after_mutation_selects_mux_row_in_mux_pins_group() {
        let mut snap = snapshot_session_with_mux();
        snap.pins.push(PinCandidate {
            id: "work-pin".to_string(),
            display_name: "Work".to_string(),
            harness: "claude-code".to_string(),
            cwd: "/p/proj".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "work".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/proj/.conspectus.toml".to_string(),
            binding: Some(PinBinding::StaleMux {
                mux: crate::model::MuxSessionId::new("work"),
            }),
        });
        let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
            snapshot: &snap,
            home: None,
            now: None,
            filter: crate::tui::RowFilter::default(),
            grouping: crate::tui::MuxGrouping::Repo,
            sort: crate::tui::Sort::Hierarchy,
        });
        let mut app = App::new(RunConfig {
            default_view: View::Mux,
            mux_grouping: crate::tui::MuxGrouping::Repo,
            ..RunConfig::defaults()
        });
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app.expanded.remove(&RowId::Synthetic("pins"));

        assert!(app.select_pin_after_mutation("work-pin"));
        assert!(app.expanded.contains(&RowId::Synthetic("pins")));
        assert!(matches!(
            app.selection(),
            Some(RowId::MuxSession(NodeId::MuxSession(id))) if id.native_id == "work"
        ));
        let first_visible_pin_row = app
            .visible_rows()
            .into_iter()
            .find(|row| row_pin_id(row) == Some("work-pin"))
            .expect("visible pinned mux row");
        assert_eq!(Some(&first_visible_pin_row.id), app.selection());
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

        let snap = app.graph_db().unwrap().snapshot().clone();
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
    fn nav_down_past_duplicate_row_id_advances_to_the_following_row() {
        // Regression: the mux view emits the same agent-session
        // RowId under every candidate mux when the resolver hasn't
        // picked. Before the duplicate-RowId tiebreaker, `NavDown`
        // from the second copy snapped back to the row after the
        // first copy because `move_selection` looked up the current
        // position with `.position(...)`, which returned the first
        // occurrence. With the tiebreaker, the cursor advances to
        // the row *immediately following* the second copy as
        // expected. Use a hand-built RowTree so the test does not
        // depend on mux row-builder details.
        use crate::tui::rows::{GroupRow, Row, RowId, RowKind};

        let group_row = |id: NodeId, label: &str| Row {
            id: RowId::Group(id.clone()),
            depth: 0,
            expandable: false,
            kind: RowKind::Group(GroupRow {
                display_path: label.to_string(),
                primary_node: Some(id),
                is_launch_context: false,
            }),
        };
        let workspace = |key: &str| NodeId::Workspace(WorkspaceId::new(key));
        let dup_id = workspace("dup");
        let tree = crate::tui::rows::RowTree {
            view: crate::tui::rows::ViewLabel::Mux,
            rows: vec![
                group_row(workspace("a"), "a"),
                group_row(dup_id.clone(), "dup-first"),
                group_row(dup_id.clone(), "dup-second"),
                group_row(workspace("c"), "c"),
            ],
        };

        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&GraphSnapshot::empty()),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });

        // Step onto the first duplicate, then the second.
        app.update(Msg::NavDown);
        app.update(Msg::NavDown);
        assert_eq!(app.last_visible_index.get(), Some(2));
        assert_eq!(app.selection.as_ref(), Some(&RowId::Group(dup_id.clone())));

        // From the second duplicate, NavDown must advance to the
        // row *after* it, not snap back to the row after the first
        // copy.
        app.update(Msg::NavDown);
        assert_eq!(app.last_visible_index.get(), Some(3));
        assert_eq!(
            app.selection.as_ref(),
            Some(&RowId::Group(workspace("c"))),
            "NavDown from the second duplicate must land on the next row, not snap to the row after the first copy",
        );
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
    fn expand_row_opens_then_no_ops_when_already_open() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        // Selection starts on the first group row.
        let group_id = app.selection().cloned().unwrap();
        assert!(matches!(group_id, RowId::Group(_)));
        // Force-collapse to mirror the user pressing `h` on an
        // already-expanded row.
        app.update(Msg::CollapseRow);
        let visible_after_collapse = app.visible_rows_owned().len();
        app.update(Msg::ExpandRow);
        let visible_after_expand = app.visible_rows_owned().len();
        assert!(
            visible_after_expand > visible_after_collapse,
            "expand reveals children"
        );
        // Repeated expand is a no-op (it doesn't re-collapse).
        app.update(Msg::ExpandRow);
        assert_eq!(app.visible_rows_owned().len(), visible_after_expand);
    }

    #[test]
    fn collapse_row_hides_descendants_and_then_no_ops() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        let visible_before = app.visible_rows_owned().len();
        app.update(Msg::CollapseRow);
        let visible_after = app.visible_rows_owned().len();
        assert!(visible_after < visible_before, "collapse hides descendants");
        // Repeated collapse is a no-op.
        app.update(Msg::CollapseRow);
        assert_eq!(app.visible_rows_owned().len(), visible_after);
    }

    #[test]
    fn expand_collapse_no_op_on_leaf_row() {
        let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
        // Move past the group row to the leaf session row.
        app.update(Msg::NavDown);
        let leaf_id = app.selection().cloned().unwrap();
        assert!(matches!(leaf_id, RowId::AgentSession(_)));
        let visible = app.visible_rows_owned().len();
        app.update(Msg::ExpandRow);
        assert_eq!(app.visible_rows_owned().len(), visible);
        app.update(Msg::CollapseRow);
        assert_eq!(app.visible_rows_owned().len(), visible);
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
    fn adjust_explorer_scroll_keeps_cursor_in_viewport() {
        let app = App::new(RunConfig::defaults());
        assert_eq!(app.adjust_explorer_scroll(0, 0, 5), 0);
        assert_eq!(app.adjust_explorer_scroll(10, 10, 5), 6);
        assert_eq!(app.adjust_explorer_scroll(4, 4, 5), 4);
    }

    #[test]
    fn adjust_explorer_scroll_holds_when_cursor_inside_viewport() {
        let app = App::new(RunConfig::defaults());
        app.adjust_explorer_scroll(10, 10, 5);
        assert_eq!(app.explorer_scroll(), 6);
        assert_eq!(app.adjust_explorer_scroll(8, 8, 5), 6);
        assert_eq!(app.adjust_explorer_scroll(7, 7, 5), 6);
        assert_eq!(app.adjust_explorer_scroll(10, 10, 5), 6);
    }

    #[test]
    fn adjust_explorer_scroll_with_zero_viewport_does_nothing() {
        let app = App::new(RunConfig::defaults());
        app.adjust_explorer_scroll(10, 10, 5);
        let before = app.explorer_scroll();
        let returned = app.adjust_explorer_scroll(99, 99, 0);
        assert_eq!(returned, before);
        assert_eq!(app.explorer_scroll(), before);
    }

    #[test]
    fn adjust_explorer_scroll_keeps_wrapped_cursor_line_fully_visible() {
        // Regression: when the cursor's logical line wraps to 2+
        // rendered rows, only feeding the line's start row left
        // the trailing wrap rows below the viewport bottom. The
        // span-aware API uses the cursor's last row to drive the
        // "scroll down" branch so a 2-row wrapped cursor line at
        // the bottom of the content advances the offset enough
        // for both rows to fit.
        let app = App::new(RunConfig::defaults());
        // Viewport 5 rows. Cursor's line starts at row 9 and
        // wraps to 2 rows (occupies 9 and 10). The offset must
        // advance to 6 so both 9 and 10 fit in [6, 10].
        assert_eq!(app.adjust_explorer_scroll(9, 10, 5), 6);
        assert_eq!(app.explorer_scroll(), 6);
        // Single-row cursor at the same row keeps the older
        // tighter behavior (offset = 5).
        let app = App::new(RunConfig::defaults());
        assert_eq!(app.adjust_explorer_scroll(9, 9, 5), 5);
    }

    #[test]
    fn scenario_ambiguous_mux_session_is_leaf_after_adr_0071() {
        // ADR 0071: ambiguous mux candidates no longer expand a
        // per-session subtree; the chip stays but the row is a
        // leaf. The catalog of muxes lives on the shared-ancestor
        // group's detail pane via `ambiguous_muxes_for_group`.
        let (mut app, _) = scenario_app("ambiguous-mux");
        select_session(&mut app, "ambiguous");

        let session_row_id = app.selection().cloned().expect("session selected");
        let session_row = app
            .visible_rows()
            .iter()
            .find(|row| row.id == session_row_id)
            .cloned()
            .expect("session row visible");
        assert!(
            !session_row.expandable,
            "ambiguous session row stops being expandable after ADR 0071",
        );
        assert!(
            !app.visible_rows()
                .iter()
                .any(|row| matches!(row.kind, RowKind::AgentSessionMuxCandidate(_))),
            "no candidate child rows after ADR 0071",
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
        let snap = app.graph_db().unwrap().snapshot().clone();
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

    use crate::model::{
        Confidence, LinkEndpoint, LinkState, MuxSessionNode, RelationKind, SourceMetadata,
    };
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
    fn explorer_home_and_end_snap_cursor_to_first_and_last_row() {
        // H-OBS-007: `g`/`Home` and `G`/`End` snap the explorer
        // cursor to its first / last row when the right pane has
        // focus. Reducer-level test: dispatching the messages
        // directly drives the cursor regardless of which pane has
        // focus (the focus check happens in `remap_for_focus`).
        let mut app = app_for_explorer();
        let row_count = app.explorer().expect("state").rows().len();
        assert!(row_count > 1, "fixture should expose multiple rows");

        // Walk the cursor down a couple of rows, then End it.
        app.update(Msg::ExplorerNavDown);
        app.update(Msg::ExplorerNavDown);
        app.update(Msg::ExplorerEnd);
        assert_eq!(
            app.explorer().expect("state").cursor,
            row_count - 1,
            "ExplorerEnd should snap cursor to the last row",
        );

        // Home brings it back to the top.
        app.update(Msg::ExplorerHome);
        assert_eq!(
            app.explorer().expect("state").cursor,
            0,
            "ExplorerHome should snap cursor to the first row",
        );
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
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
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
    fn explorer_drill_mirror_sync_keeps_left_pane_when_neighbor_has_no_row() {
        // T8-035: drilling from a session into its mux while the
        // left pane is in the sessions view should preserve the
        // prior selection because the sessions view doesn't carry
        // a MuxSession row. The hop still records the prior
        // selection so Backspace can restore it.
        let mut app = app_for_explorer();
        let pre_drill_selection = app
            .selection()
            .expect("session selection in app_for_explorer")
            .clone();
        let target_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
            .expect("link row");
        for _ in 0..target_idx {
            app.update(Msg::ExplorerNavDown);
        }
        app.update(Msg::ExplorerActivate);
        // Sessions view doesn't render the mux as a row, so the
        // left selection should stay put.
        assert_eq!(
            app.selection().expect("selection after drill"),
            &pre_drill_selection,
            "left selection should be preserved when the neighbor has no row",
        );
        // But the explorer should still be focused on the mux —
        // mirror sync's missing-row fallback only affects the left
        // pane.
        assert_eq!(
            app.explorer().expect("state").view.kind_label,
            "mux_session"
        );
        let hop = app
            .explorer()
            .expect("state")
            .breadcrumb
            .last()
            .expect("one hop after drill")
            .clone();
        assert_eq!(hop.left_pane_selection.as_ref(), Some(&pre_drill_selection));
    }

    #[test]
    fn explorer_drill_mirrors_left_pane_to_neighbor_when_present_in_tree() {
        // T8-035: when the drilled neighbor *does* have a row in
        // the current view (here: drilling from one agent session
        // to a sibling agent session via `ParentSession`), the
        // left pane should move to it.
        let mut snap = snapshot_session_with_mux();
        // Add a second agent session and a parent_session link
        // session_a → session_b so the explorer's downstream group
        // exposes the sibling as a drillable neighbor.
        let session_b = AgentSessionNode {
            id: AgentSessionId::new("claude-code", "/state", "child"),
            harness_key: "claude-code".to_string(),
            cwd: Some("/p/proj".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(1_700_000_000),
            session_kind: None,
        };
        let parent_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
        let child_id = NodeId::AgentSession(session_b.id.clone());
        snap.nodes.push(GraphNode::AgentSession(session_b));
        snap.candidate_links.push(crate::model::GraphLink {
            id: "sibling".to_string(),
            source: parent_id.clone(),
            target: LinkEndpoint::Node {
                id: child_id.clone(),
            },
            relation: RelationKind::ParentSession,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        let snap = resolve_snapshot(snap);
        let tree = build_tree(&snap);
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snap),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        // Select the parent session row. Use the underlying tree
        // rather than visible_rows because the parent may sit under
        // a not-yet-expanded group; set_selection accepts any row
        // that exists in the flat tree.
        let parent_row = app
            .tree()
            .rows
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(s) if s.session.session_key == "abc" => Some(row.id.clone()),
                _ => None,
            })
            .expect("parent agent session row in tree");
        app.set_selection(parent_row.clone());
        app.update(Msg::CycleFocus);
        // Walk to a link row whose neighbor is the child session.
        let rows = app.explorer().expect("state").rows();
        let link_idx = rows
            .iter()
            .enumerate()
            .find_map(|(idx, row)| match row {
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                    if app.explorer().expect("state").view.drill_target(row)
                        == Some(child_id.clone()) =>
                {
                    Some(idx)
                }
                _ => None,
            })
            .expect("link row drilling into the child session");
        for _ in 0..link_idx {
            app.update(Msg::ExplorerNavDown);
        }
        app.update(Msg::ExplorerActivate);
        let post = app.selection().expect("selection after drill").clone();
        let child_row = app
            .tree()
            .rows
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(s) if s.session.session_key == "child" => {
                    Some(row.id.clone())
                }
                _ => None,
            })
            .expect("child agent session row in tree");
        assert_eq!(
            post, child_row,
            "left pane should mirror the drilled neighbor when it has a row",
        );
        // Right pane still on the child session.
        assert_eq!(
            app.explorer().expect("state").view.focused,
            child_id,
            "explorer should still focus the drilled neighbor",
        );
    }

    #[test]
    fn explorer_backspace_restores_left_pane_selection() {
        // T8-035: Backspace should pop the hop, restore the prior
        // left-pane selection, and refocus the explorer on the
        // pre-drill node.
        let mut app = app_for_explorer();
        let pre_drill_selection = app
            .selection()
            .expect("session selection in app_for_explorer")
            .clone();
        let target_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
            .expect("link row");
        for _ in 0..target_idx {
            app.update(Msg::ExplorerNavDown);
        }
        app.update(Msg::ExplorerActivate);
        // Backspace.
        app.update(Msg::ExplorerBack);
        assert_eq!(
            app.selection().expect("selection after backspace"),
            &pre_drill_selection,
            "left pane should be restored to the pre-drill row",
        );
        assert_eq!(
            app.explorer().expect("state").view.kind_label,
            "agent_session",
            "right pane should be restored to the pre-drill node",
        );
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
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
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
    fn explorer_back_with_no_breadcrumb_and_left_focus_surfaces_status_hint() {
        let mut app = app_for_explorer();
        assert_eq!(app.focus(), Focus::Left);
        app.update(Msg::ExplorerBack);
        assert!(
            app.status_message()
                .map(|s| s.contains("no drill history"))
                .unwrap_or(false)
        );
        // Focus stays put when there's nothing to back out of.
        assert_eq!(app.focus(), Focus::Left);
    }

    #[test]
    fn explorer_back_with_no_breadcrumb_and_right_focus_arms_then_shifts_focus_on_second_press() {
        // Backspace on the right pane with an empty drilldown stack
        // requires a confirmation press before backing out of the
        // pane entirely: the first press surfaces a hint, the second
        // performs the focus shift. This matches "press Backspace
        // twice to leave the right pane" UX.
        let mut app = app_for_explorer();
        app.update(Msg::CycleFocus);
        assert_eq!(app.focus(), Focus::Right);
        // First press: arms the shift and surfaces the hint.
        app.update(Msg::ExplorerBack);
        assert_eq!(app.focus(), Focus::Right);
        assert!(
            app.status_message()
                .map(|s| s.contains("press Backspace again"))
                .unwrap_or(false),
            "first backspace should surface the confirmation hint; got: {:?}",
            app.status_message()
        );
        // Second press: actually shifts focus, clears the hint.
        app.update(Msg::ExplorerBack);
        assert_eq!(app.focus(), Focus::Left);
        assert!(app.status_message().is_none());
    }

    #[test]
    fn explorer_back_armed_state_clears_on_intervening_message() {
        // The "press Backspace again" arming only survives across
        // consecutive Backspace presses. Any intervening message
        // (e.g. navigation, focus cycle) should reset it so the next
        // Backspace once again surfaces the hint instead of jumping
        // straight to the focus shift.
        let mut app = app_for_explorer();
        app.update(Msg::CycleFocus);
        assert_eq!(app.focus(), Focus::Right);
        app.update(Msg::ExplorerBack);
        assert!(
            app.status_message()
                .map(|s| s.contains("press Backspace again"))
                .unwrap_or(false)
        );
        // Intervening navigation cancels the arming.
        app.update(Msg::ExplorerNavDown);
        // Next Backspace should re-arm, not shift focus.
        app.update(Msg::ExplorerBack);
        assert_eq!(app.focus(), Focus::Right);
        assert!(
            app.status_message()
                .map(|s| s.contains("press Backspace again"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn explorer_back_unwinds_drill_then_arms_then_shifts_focus() {
        // T8-031 follow-up: with one drilldown hop on the stack, three
        // Backspace taps now (1) pop the hop, (2) arm the focus shift
        // with a hint, and (3) shift focus to the left pane.
        let mut app = app_for_explorer();
        app.update(Msg::CycleFocus);
        assert_eq!(app.focus(), Focus::Right);
        let link_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
            .expect("link row");
        for _ in 0..link_idx {
            app.update(Msg::ExplorerNavDown);
        }
        let before = app.explorer().expect("state").view.focused.clone();
        app.update(Msg::ExplorerActivate);
        assert_ne!(app.explorer().expect("state").view.focused, before);
        // First backspace pops the drilldown hop; focus stays right.
        app.update(Msg::ExplorerBack);
        assert_eq!(app.explorer().expect("state").view.focused, before);
        assert_eq!(app.focus(), Focus::Right);
        // Second backspace at the empty stack arms the focus shift.
        app.update(Msg::ExplorerBack);
        assert_eq!(app.focus(), Focus::Right);
        assert!(
            app.status_message()
                .map(|s| s.contains("press Backspace again"))
                .unwrap_or(false)
        );
        // Third backspace shifts focus to the left pane.
        app.update(Msg::ExplorerBack);
        assert_eq!(app.focus(), Focus::Left);
    }

    #[test]
    fn edge_meta_visibility_defaults_to_run_config_value_and_toggles() {
        // T8-042: edge_meta_visible starts from `RunConfig.show_edge_meta`
        // and Msg::ToggleEdgeMeta flips it with a status hint.
        let app = App::new(RunConfig::defaults());
        assert!(
            !app.edge_meta_visible(),
            "RunConfig::defaults() should hide edge meta by default",
        );
        let mut config = RunConfig::defaults();
        config.show_edge_meta = true;
        let app_with_meta = App::new(config);
        assert!(
            app_with_meta.edge_meta_visible(),
            "config knob should set the initial state",
        );
        let mut app = app_for_explorer();
        assert!(!app.edge_meta_visible());
        app.update(Msg::ToggleEdgeMeta);
        assert!(app.edge_meta_visible());
        assert!(
            app.status_message()
                .map(|s| s.contains("edge meta visible"))
                .unwrap_or(false)
        );
        app.update(Msg::ToggleEdgeMeta);
        assert!(!app.edge_meta_visible());
        assert!(
            app.status_message()
                .map(|s| s.contains("edge meta hidden"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn explorer_toggle_full_detail_swaps_core_for_all_fields() {
        // T8-034: toggling Expanded Node Detail should swap the
        // Node-zone field rows for the per-kind `all_fields` set.
        // app_for_explorer focuses on an agent session, whose
        // all_fields is a superset of core_fields.
        let mut app = app_for_explorer();
        let core_count = app.explorer().expect("state").view.core_fields.len();
        let all_count = app.explorer().expect("state").view.all_fields.len();
        assert!(
            all_count > core_count,
            "test premise: agent_session should carry extras",
        );
        let rows_before = app.explorer().expect("state").rows();
        let node_field_count_before = rows_before
            .iter()
            .filter(|r| matches!(r, ExplorerRow::NodeField { .. }))
            .count();
        assert_eq!(node_field_count_before, core_count);

        app.update(Msg::ExplorerToggleFullDetail);
        assert!(app.explorer().expect("state").full_detail_expanded);
        let rows_after = app.explorer().expect("state").rows();
        let node_field_count_after = rows_after
            .iter()
            .filter(|r| matches!(r, ExplorerRow::NodeField { .. }))
            .count();
        assert_eq!(node_field_count_after, all_count);

        // Toggle back.
        app.update(Msg::ExplorerToggleFullDetail);
        assert!(!app.explorer().expect("state").full_detail_expanded);
        let rows_back = app.explorer().expect("state").rows();
        let node_field_count_back = rows_back
            .iter()
            .filter(|r| matches!(r, ExplorerRow::NodeField { .. }))
            .count();
        assert_eq!(node_field_count_back, core_count);
    }

    #[test]
    fn explorer_full_detail_resets_on_drill_and_restores_on_backspace() {
        // T8-034: the toggle is per-focused-node — drilling into a
        // neighbor resets it, and Backspace restores the prior
        // node's toggle state.
        let mut app = app_for_explorer();
        // Turn on Expanded Detail on the original node.
        app.update(Msg::ExplorerToggleFullDetail);
        assert!(app.explorer().expect("state").full_detail_expanded);
        // Walk to the first link row and drill.
        let link_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
            .expect("link row");
        for _ in 0..link_idx {
            app.update(Msg::ExplorerNavDown);
        }
        app.update(Msg::ExplorerActivate);
        // Drilled state should default back to compact.
        assert!(!app.explorer().expect("state").full_detail_expanded);
        // Backspace should restore the prior toggle state.
        app.update(Msg::ExplorerBack);
        assert!(
            app.explorer().expect("state").full_detail_expanded,
            "Backspace should restore the prior node's Expanded Detail toggle",
        );
    }

    #[test]
    fn explorer_toggle_group_only_acts_on_other_header() {
        // ADR 0074: toggling expansion only makes sense on the
        // `Other` zone header now that the per-relation sub-headers
        // are gone. Triggering the toggle from a validated link row
        // (or any other row kind) surfaces a status hint instead of
        // silently doing nothing.
        let mut app = app_for_explorer();
        let link_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
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
    fn scenario_process_cardinality_exposes_upstream_process_groups() {
        // T8-031: the process-cardinality dev scenario is the
        // canonical "messy" setup with one preferred process and one
        // candidate runner-up. With the new explorer, those should
        // both surface as Upstream groups on the agent session.
        let (mut app, _snap) = scenario_app("process-cardinality");
        let target = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(_) => Some(row.id.clone()),
                _ => None,
            })
            .expect("an agent session row in the scenario");
        app.set_selection(target);
        let state = app.explorer().expect("explorer for session");
        let labels: Vec<&str> = state
            .view
            .relationships
            .groups
            .iter()
            .filter(|g| g.direction == crate::tui::explorer::Direction::Upstream)
            .map(|g| g.relation.snake_case())
            .collect();
        assert!(
            labels.contains(&"process_identifies_session")
                || labels.contains(&"process_candidates_session"),
            "process-cardinality should expose process groups upstream: {labels:?}"
        );
    }

    #[test]
    fn scenario_codex_fd_current_exposes_session_linked_groups() {
        // T8-031: codex-fd-current is the canonical "fd evidence
        // outranks stale launch command" setup. The detail explorer
        // should show the linked mux as a downstream group on the
        // agent session so an operator can drill into it manually.
        let (mut app, _snap) = scenario_app("codex-fd-current");
        let target = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(_) => Some(row.id.clone()),
                _ => None,
            })
            .expect("an agent session row in the scenario");
        app.set_selection(target);
        let state = app.explorer().expect("explorer for session");
        let downstream_kinds: Vec<&str> = state
            .view
            .relationships
            .groups
            .iter()
            .filter(|g| g.direction == crate::tui::explorer::Direction::Downstream)
            .map(|g| g.neighbor_kind.as_str())
            .collect();
        assert!(
            downstream_kinds.contains(&"mux_session"),
            "codex-fd-current should link the session to a mux downstream: {downstream_kinds:?}"
        );
    }

    #[test]
    fn scenario_ambiguous_mux_exposes_two_candidate_muxes() {
        // T8-031: ambiguous-mux carries two plausible tmux sessions
        // for one agent. The explorer should surface both as
        // selectable rows in a single downstream group so operators
        // can drill into either candidate from the detail pane.
        let (mut app, _snap) = scenario_app("ambiguous-mux");
        let target = app
            .visible_rows()
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(_) => Some(row.id.clone()),
                _ => None,
            })
            .expect("an agent session row in the scenario");
        app.set_selection(target);
        let state = app.explorer().expect("explorer for session");
        let total_mux_links: usize = state
            .view
            .relationships
            .groups
            .iter()
            .filter(|g| {
                g.direction == crate::tui::explorer::Direction::Downstream
                    && g.neighbor_kind == "mux_session"
            })
            .map(|g| g.link_count())
            .sum();
        assert!(
            total_mux_links >= 2,
            "ambiguous-mux should surface two mux candidates in downstream groups; got {total_mux_links}"
        );
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
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
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

    // ------------------------------------------------------------------
    // T8-040: Enter-to-copy on Node-zone field rows + `i` for full id.
    // ------------------------------------------------------------------

    #[test]
    fn explorer_copy_target_returns_value_on_node_field_row() {
        let app = app_for_explorer();
        // Cursor defaults to the first Node-zone field row, which is
        // the focused agent session's `id` field.
        let (label, value) = app
            .explorer_copy_target()
            .expect("Node-zone field rows have a copy target");
        assert!(!label.is_empty(), "label must caption the toast");
        assert!(!value.is_empty(), "value must be the clipboard payload");
    }

    #[test]
    fn explorer_copy_target_is_none_when_cursor_walks_onto_link_row() {
        // T8-040: Enter on link rows still drills; the copy seam must
        // refuse so the runtime falls through to ExplorerActivate.
        let mut app = app_for_explorer();
        let link_idx = app
            .explorer()
            .expect("explorer state present")
            .rows()
            .iter()
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
            .expect("link row");
        for _ in 0..link_idx {
            app.update(Msg::ExplorerNavDown);
        }
        assert!(app.explorer_copy_target().is_none());
    }

    #[test]
    fn selected_session_id_returns_full_id_for_agent_session_row() {
        let app = app_for_explorer();
        let (label, value) = app
            .selected_session_id()
            .expect("agent session selection has a full id");
        assert_eq!(label, "copied: agent_session id");
        // Display form is `agent_session:<harness>:<scope>:<key>`
        // per NodeId / AgentSessionId — full id, not a short form.
        assert!(
            value.starts_with("agent_session:"),
            "expected full id prefix, got {value:?}",
        );
    }

    #[test]
    fn selected_session_id_is_none_on_non_session_selection() {
        let mut app = app_for_explorer();
        // Drop the selection so we cover the "nothing selected" arm
        // (which is the same as a non-session selection from the
        // runtime's perspective).
        app.selection = None;
        assert!(app.selected_session_id().is_none());
    }

    #[test]
    fn post_toast_supersedes_prior_toast() {
        // H-WIDG-003 contract: posting a new toast drains any prior
        // queued toast so the newer feedback is the one rendered.
        // Under the upstream engine the queue length stays at 1
        // after a second post even though the engine itself supports
        // queueing — `engine_dismiss_all` runs before each show.
        let mut app = app_for_explorer();
        assert!(!app.toast().has_toast());
        app.post_toast("copied: cwd");
        assert!(app.toast().has_toast());
        assert_eq!(app.toast().queue_len(), 1);
        app.post_toast("copied: id");
        assert_eq!(
            app.toast().queue_len(),
            1,
            "newer toast must drain the queue"
        );
        assert_eq!(app.toast().current_message(), Some("copied: id"));
    }

    #[test]
    fn switch_to_view_persists_through_enabled_cache() {
        // F8-013: when persistence is enabled, every view switch must
        // funnel through `crate::tui_state::write_tui_state`. We pin
        // the seam by enabling a cache pointed at a tempdir and
        // asserting the on-disk file appears with the new view.
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = crate::tui_state::TuiStateCache::default().with_xdg_state_home(dir.path());
        let cache_for_assert = cache.clone();

        let mut app = App::new(RunConfig::defaults());
        app.enable_view_persistence(cache);
        app.switch_to_view(View::Mux);

        assert_eq!(
            crate::tui_state::read_last_view(&cache_for_assert),
            Some(View::Mux),
            "switch_to_view must persist through the enabled cache",
        );
    }

    #[test]
    fn restore_persisted_state_applies_state_and_mirrors_config() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = crate::tui_state::TuiStateCache::default().with_xdg_state_home(dir.path());
        let mut persisted = crate::tui_state::PersistedState {
            last_view: Some(View::Sessions),
            sort: Some(crate::tui::Sort::Recency),
            view_states: BTreeMap::new(),
        };
        let filter = crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
            ..crate::filter::RowFilter::default()
        };
        persisted.view_states.insert(
            View::Sessions,
            crate::tui_state::PersistedViewSlot {
                filter: filter.clone(),
                grouping: Some(crate::tui::Grouping::Sessions(
                    crate::tui::SessionsGrouping::Repo,
                )),
            },
        );
        crate::tui_state::write_tui_state(&cache, &persisted).expect("seed state");

        let mut app = App::new(RunConfig::defaults());
        app.enable_view_persistence(cache);
        app.restore_persisted_state();

        assert_eq!(app.sort(), crate::tui::Sort::Recency);
        assert_eq!(app.config().default_sort, crate::tui::Sort::Recency);
        assert_eq!(app.filter(), &filter);
        assert_eq!(app.config().initial_filter, filter);
        assert_eq!(
            app.grouping(),
            crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::Repo)
        );
        assert_eq!(
            app.config().sessions_grouping,
            crate::tui::SessionsGrouping::Repo
        );
    }

    #[test]
    fn restore_persisted_state_preserves_explicit_cli_overrides() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = crate::tui_state::TuiStateCache::default().with_xdg_state_home(dir.path());
        let persisted_filter = crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
            ..crate::filter::RowFilter::default()
        };
        let mut persisted = crate::tui_state::PersistedState {
            last_view: Some(View::Sessions),
            sort: Some(crate::tui::Sort::Recency),
            view_states: BTreeMap::new(),
        };
        persisted.view_states.insert(
            View::Sessions,
            crate::tui_state::PersistedViewSlot {
                filter: persisted_filter,
                grouping: Some(crate::tui::Grouping::Sessions(
                    crate::tui::SessionsGrouping::Repo,
                )),
            },
        );
        crate::tui_state::write_tui_state(&cache, &persisted).expect("seed state");

        let cli_filter = crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["claude-code"])),
            ..crate::filter::RowFilter::default()
        };
        let mut config = RunConfig::defaults();
        config.default_sort = crate::tui::Sort::Hierarchy;
        config.initial_filter = cli_filter.clone();
        config.sessions_grouping = crate::tui::SessionsGrouping::Workspace;
        config.explicit_sort = true;
        config.explicit_filter = true;
        config.explicit_grouping = true;

        let mut app = App::new(config);
        app.enable_view_persistence(cache);
        app.restore_persisted_state();

        assert_eq!(app.sort(), crate::tui::Sort::Hierarchy);
        assert_eq!(app.config().default_sort, crate::tui::Sort::Hierarchy);
        assert_eq!(app.filter(), &cli_filter);
        assert_eq!(app.config().initial_filter, cli_filter);
        assert_eq!(
            app.grouping(),
            crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::Workspace)
        );
        assert_eq!(
            app.config().sessions_grouping,
            crate::tui::SessionsGrouping::Workspace
        );
    }

    #[test]
    fn switch_to_view_without_persistence_does_not_write() {
        // Negative pin: when persistence is disabled (the default,
        // matching snapshot mode and `--no-resume-view`), the on-disk
        // file must not appear. Guards against accidental
        // unconditional writes leaking into snapshot tests.
        let dir = tempfile::TempDir::new().expect("tempdir");
        let cache = crate::tui_state::TuiStateCache::default().with_xdg_state_home(dir.path());

        let mut app = App::new(RunConfig::defaults());
        // Deliberately skip `enable_view_persistence`.
        app.switch_to_view(View::Prs);

        assert!(
            crate::tui_state::read_last_view(&cache).is_none(),
            "view switch without enabled persistence must not write",
        );
    }
}
