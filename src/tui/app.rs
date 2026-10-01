//! Pure app state.
//!
//! Per ADR 0024 and ADR 0085, [`App::update`] is synchronous and
//! free of I/O: the runtime translates terminal events,
//! background-task results, and timer ticks into [`Msg`]s and feeds
//! them in. The reducer owns the row tree, expanded set, selection
//! (retained across refreshes), panel focus, overlays, and scroll
//! state.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;
use std::time::Instant;

use crate::model::{Diagnostic, GraphNode, GraphSnapshot, MuxSessionId, NodeId, PinBinding, PinId};
use crate::tui::detail::{DetailInputs, NodeDetail, build_node_detail};
use crate::tui::explorer::{
    BreadcrumbHop, ExplorerInputs, ExplorerRow, ExplorerRowKey, NodeView, build_node_view,
};
use crate::tui::preview::{PreviewContent, PreviewEntry, PreviewStore};
use crate::tui::rows::{Row, RowId, RowKind, RowTree};
use crate::tui::widgets::controls::harness_options;
use crate::tui::widgets::path_omnibox::PathCandidate;
use crate::tui::widgets::pins::{
    PinBindOption, PinCreateDefaults, PinCreateMode, PinMutationTarget,
};
use crate::tui::{RunConfig, View};

mod explorer_nav;
mod msg;
mod overlays;
mod pins;
mod tree;

pub use msg::Msg;

/// Cheap-to-clone, reference-counted handle to the App's resolved
/// [`GraphSnapshot`]. Read consumers borrow it via [`Self::snapshot`].
#[derive(Clone)]
pub struct SnapshotHandle(Rc<crate::model::GraphSnapshot>);

impl SnapshotHandle {
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

impl fmt::Debug for SnapshotHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SnapshotHandle")
            .field(&"<snapshot>")
            .finish()
    }
}

impl PartialEq for SnapshotHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Per-provider availability status for the right-side status-bar
/// chips.
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
    /// Latest resolved graph snapshot. `None` before the first
    /// `SetData`.
    handle: Option<SnapshotHandle>,
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
    /// Whether linked-entity summary rows in the right-panel detail
    /// are expanded in place.
    detail_links_expanded: bool,
    /// Right-panel graph explorer state. Carries
    /// the focused node view, the navigation cursor, group expansion,
    /// and the breadcrumb stack for drilldown. `None` until the
    /// reducer has resolved a selection into a node view.
    explorer: Option<ExplorerState>,
    /// Which panel currently consumes navigation keys.
    focus: Focus,
    /// Whether the explorer's link rows render the trailing
    /// `provenance · confidence · state` meta line.
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
    /// In-flight async operations the status bar surfaces as
    /// animated spinner chips. Ordered `Vec` so the
    /// render order matches the emit order; entries are keyed by
    /// `InFlightKind` for idempotent start/finish semantics.
    in_flight_ops: Vec<InFlightOp>,
    /// Cache of recent tmux pane captures, keyed by mux id. The
    /// renderer reads this for the right-panel preview when the
    /// selection points at a muxed agent session or a mux node.
    preview_store: PreviewStore,
    /// Left-panel vertical scroll offset, in rendered lines. The
    /// renderer reconciles this each frame via
    /// [`App::adjust_left_scroll`] so the selected row stays
    /// visible: draw measures the viewport and dispatches
    /// `Msg::LeftViewportChanged` (ADR 0085 contract 5).
    left_scroll: u16,
    /// Right-panel explorer scroll offset, in rendered lines. Used
    /// to keep the explorer cursor visible inside the header section
    /// when the Related list grows past the section's height (the
    /// preview zone below reserves a minimum). Reconciled each
    /// frame via [`App::adjust_explorer_scroll`], mirroring the
    /// left pane's pattern. Does not affect
    /// [`Self::preview_scroll`], which scrolls the preview body
    /// independently.
    explorer_scroll: u16,
    /// Last visible-row index the selection landed on, used as a
    /// tiebreaker when the same `RowId` appears in multiple visible
    /// positions (e.g. the mux view lists the same ambiguously-
    /// attached session under every candidate mux). Without this,
    /// navigation looks up "current position" with `.position(...)`
    /// which always returns the first occurrence, and `j` from a
    /// later duplicate snaps the cursor back to the row after the
    /// first one.
    last_visible_index: Option<usize>,
    /// Pin id waiting for a second `Delete` press. This gives pin
    /// removal a confirmation step without a full modal.
    pending_pin_remove: Option<String>,
    /// Open overlays as a stack (ADR 0085 contract 3). Input routes
    /// to the top entry first, `draw` renders bottom-to-top, and
    /// commit / close outcomes pop the top. Grows one variant per
    /// overlay migration wave — see [`crate::tui::Modal`]. The
    /// remaining `Option<...>` fields below still host overlays
    /// that haven't migrated yet; each wave deletes one field and
    /// moves the state to a `Modal` variant.
    modal_stack: Vec<crate::tui::Modal>,
    /// Active transient toast. The
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
    /// Currently active view (ADR 0085 contract 4). Seeded from
    /// [`RunConfig::default_view`] at startup; after that, the
    /// reducer is the sole owner. `RunConfig` remains as an
    /// initial-values source and never re-reads from here — the
    /// TUI's projection state lives on `App`, not on the config.
    active_view: View,
    /// Global sort toggle (ADR 0031). Per-view state covers
    /// filter/grouping/expanded; sort stays global because the
    /// recency-vs-hierarchy choice is view-independent in operator
    /// practice. Seeded from [`RunConfig::default_sort`].
    sort: super::Sort,
    /// Mux-view recency basis. Selects which epoch
    /// `Sort::Recency` orders the mux tree by (activity / created /
    /// last-attached). Mux-scoped: no other view reads it. Seeded from
    /// [`RunConfig::default_mux_recency`].
    mux_recency: super::MuxRecency,
    /// Active row filter (ADR 0031). Mirrors the active
    /// view's slot in `view_states` so callers don't pay a map
    /// lookup per read. Kept in sync via `switch_to_view` /
    /// `Msg::SetFilter`.
    filter: crate::filter::RowFilter,
    /// Active grouping (ADR 0031). Same caching pattern as
    /// `filter` — mirrors the active view's slot.
    grouping: super::Grouping,
    /// Saved UI state for views the operator is not currently
    /// looking at (ADR 0031). On view switch the active
    /// slot is saved here and the target slot loaded into the
    /// active fields. Sort stays global, so it lives on `App`
    /// rather than per-view.
    view_states: BTreeMap<View, ViewStateSlot>,
    /// Optional persistence sink for the last-active view.
    /// When `Some`, every view switch best-effort writes
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

/// Discriminant for an async operation the TUI wants to signal to
/// the operator. Grows with each new async
/// surface — forge-metadata fetches, transcript loads, agent-deck
/// queries. The reducer stores at most one `InFlightOp` per kind, so
/// starting a new op with the same kind replaces any prior in-flight
/// record (used e.g. when a periodic discovery worker spawns while a
/// prior one is still in flight — the timer path already gates on
/// `pending_refresh`, but the model here is intentionally
/// idempotent).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InFlightKind {
    /// A discovery worker is scanning the local filesystem + forge +
    /// mux for a fresh graph snapshot. Emitted by `LiveMode::init`,
    /// the timer-driven refresh, and `Action::Refresh`.
    Discovery,
}

/// One in-flight async operation the status bar surfaces with a
/// spinner + label chip. `started_at` drives the spinner-frame
/// selection at draw time; the reducer stamps it via
/// `Instant::now()` when handling `Msg::InFlightStart` so the model
/// stays a pure state machine over the input Msgs.
#[derive(Debug, Clone)]
pub struct InFlightOp {
    pub kind: InFlightKind,
    pub label: String,
    pub started_at: Instant,
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
    /// set instead of the top-5 Core summary. Per-focused-
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

impl App {
    /// Build a fresh app at the start of the run.
    pub fn new(config: RunConfig) -> Self {
        let mut config = config;
        if matches!(config.sessions_grouping, super::SessionsGrouping::None) {
            config.default_sort = super::Sort::Recency;
        }
        let sort = config.default_sort;
        let mux_recency = config.default_mux_recency;
        let filter = config.initial_filter.clone();
        let active_view = config.default_view;
        let grouping = match active_view {
            View::Sessions => super::Grouping::Sessions(config.sessions_grouping),
            View::Mux => super::Grouping::Mux(config.mux_grouping),
            View::Union | View::Prs | View::Forks => super::Grouping::default_for(active_view),
        };
        let edge_meta_visible = config.show_edge_meta;
        Self {
            config,
            active_view,
            should_quit: false,
            handle: None,
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
            in_flight_ops: Vec::new(),
            preview_store: PreviewStore::new(),
            left_scroll: 0,
            explorer_scroll: 0,
            last_visible_index: None,
            pending_pin_remove: None,
            modal_stack: Vec::new(),
            toast: crate::tui::widgets::toast::ToastEngineHolder(
                crate::tui::widgets::toast::engine(),
            ),
            sort,
            mux_recency,
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
            left_scroll: self.left_scroll,
        }
    }

    /// Restore a saved view state into the active fields.
    fn restore_active_state(&mut self, slot: ViewStateSlot) {
        self.filter = slot.filter;
        self.grouping = slot.grouping;
        self.expanded = slot.expanded;
        self.selection = slot.selection;
        self.left_scroll = slot.left_scroll;
    }

    /// Switch the active view to `target`, saving the previous
    /// view's state into `view_states` and loading the target's
    /// state (or fresh defaults on first visit). Per ADR 0031 sort
    /// stays global, so callers don't touch it here.
    ///
    /// Persistence is the caller's concern — the [`Msg::SwitchView`]
    /// reducer arm emits [`Effect::Persist`] after invoking this,
    /// per ADR 0085 contract 2 Phase E.
    pub(crate) fn switch_to_view(&mut self, target: View) {
        let from = self.active_view;
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
        self.active_view = target;
        self.force_recency_for_flat_sessions();
        self.status_message = None;
    }

    /// Resolve whatever copyable value the explorer cursor points at.
    /// Returns `(label, value)` for the toast caption + clipboard
    /// payload, or `None` if the row is not a Node-zone field or the
    /// field has no value (empty or absent), so the toast never
    /// claims a copy that didn't happen.
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
                self.modal_stack.push(crate::tui::Modal::ValueModal(
                    crate::tui::widgets::value_modal::ValueModalState::new(label, value),
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
        self.last_visible_index = None;
        self.selection = Some(id);
        self.status_message = None;
        self.recompute_detail();
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

    /// Active row filter for the visible view.
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

    /// Mux-view recency basis.
    pub fn mux_recency(&self) -> super::MuxRecency {
        self.mux_recency
    }

    fn force_recency_for_flat_sessions(&mut self) {
        if matches!(
            self.grouping,
            super::Grouping::Sessions(super::SessionsGrouping::None)
        ) {
            self.sort = super::Sort::Recency;
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

    /// Enable last-active-view persistence. The runtime calls
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
        }

        // Mux recency basis: restore the operator's
        // last choice. There's no CLI flag for it, so persistence
        // always wins when present.
        if let Some(basis) = persisted.mux_recency {
            self.mux_recency = basis;
        }

        // Pre-populate view_states from persisted state.
        for (view, slot) in persisted.view_states {
            let is_active = view == self.active_view;
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
                }
                if !self.config.explicit_grouping
                    && let Some(g) = slot.grouping
                {
                    self.grouping = g;
                }
            }
        }
        self.force_recency_for_flat_sessions();
    }

    /// Build a `PersistedState` snapshot of the current app state
    /// for writing to the state file. Captures the last-active view,
    /// global sort, and per-view filter/grouping from `view_states`
    /// (plus the active view's current state, which may not yet be
    /// in `view_states`).
    pub fn build_persisted_state(&self) -> crate::tui_state::PersistedState {
        let mut view_states: BTreeMap<View, crate::tui_state::PersistedViewSlot> = BTreeMap::new();
        let active_view = self.active_view;
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
            last_view: Some(active_view),
            sort: Some(self.sort),
            mux_recency: Some(self.mux_recency),
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

    /// Currently active view (ADR 0085 contract 4). Reads from
    /// `App`, not `RunConfig`; the config's `default_view` is only
    /// consulted at startup to seed this field.
    pub fn active_view(&self) -> View {
        self.active_view
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
    /// `provenance · confidence · state` meta line.
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

    /// Latest graph snapshot, if loaded. Mostly useful to other modules
    /// that compute view-models against the same data.
    pub(crate) fn snapshot_handle(&self) -> Option<&SnapshotHandle> {
        self.handle.as_ref()
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
    /// status-bar chips.
    pub fn provider_status(&self) -> &ProviderStatus {
        &self.provider_status
    }

    /// Reason for the most recent refresh failure, if any.
    pub fn refresh_failure(&self) -> Option<&str> {
        self.refresh_failure.as_deref()
    }

    /// In-flight async operations tracked for status-bar spinner
    /// chips. Empty when nothing is in flight.
    pub fn in_flight_ops(&self) -> &[InFlightOp] {
        &self.in_flight_ops
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
    pub fn adjust_left_scroll(&mut self, selected_line: usize, viewport_height: u16) -> u16 {
        let vh = viewport_height as usize;
        if vh == 0 {
            return self.left_scroll;
        }
        let mut offset = self.left_scroll as usize;
        if selected_line < offset {
            offset = selected_line;
        } else if selected_line >= offset + vh {
            offset = selected_line + 1 - vh;
        }
        let clamped = u16::try_from(offset).unwrap_or(u16::MAX);
        self.left_scroll = clamped;
        clamped
    }

    /// Pure getter for the current left-panel scroll offset. The
    /// reducer owns updates (via
    /// [`Msg::LeftViewportChanged`]); the
    /// renderer reads this to size `Paragraph::scroll`.
    pub fn left_scroll(&self) -> u16 {
        self.left_scroll
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
        &mut self,
        cursor_first_row: usize,
        cursor_last_row: usize,
        viewport_height: u16,
    ) -> u16 {
        let vh = viewport_height as usize;
        if vh == 0 {
            return self.explorer_scroll;
        }
        let mut offset = self.explorer_scroll as usize;
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
        let clamped = u16::try_from(offset).unwrap_or(u16::MAX);
        self.explorer_scroll = clamped;
        clamped
    }

    /// Pure getter for the current explorer scroll offset. The
    /// reducer owns updates (via
    /// [`Msg::ExplorerViewportChanged`]);
    /// the renderer reads this to size `Paragraph::scroll`.
    pub fn explorer_scroll(&self) -> u16 {
        self.explorer_scroll
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
    pub fn update(&mut self, msg: Msg) -> Vec<super::Effect> {
        use super::Effect;
        // The empty-stack Backspace arming persists across consecutive
        // Backspace presses so the two-press "leave the right pane"
        // confirmation can complete. An intervening operator action
        // (navigation, focus cycle, …) clears it so the next Backspace
        // re-surfaces the hint instead of jumping straight to the focus
        // shift.
        //
        // The draw path dispatches `LeftViewportChanged` /
        // `ExplorerViewportChanged` every frame for scroll
        // reconciliation. Those are layout plumbing,
        // not operator input — and because `draw_frame` runs at the top
        // of every event-loop iteration, before the next key is polled,
        // counting them as an "intervening action" would disarm the
        // shift before the operator could ever land the second press
        // (the observed regression: the hint reappears but focus never
        // moves). Treat them as transparent so the arm survives to the
        // confirming Backspace.
        if !matches!(
            msg,
            Msg::ExplorerBack
                | Msg::LeftViewportChanged { .. }
                | Msg::ExplorerViewportChanged { .. }
        ) {
            self.explorer_back_armed = false;
        }
        let mut effects: Vec<Effect> = Vec::new();
        match msg {
            Msg::Quit => {
                self.should_quit = true;
                effects.push(Effect::Quit);
            }
            Msg::SetData {
                snapshot,
                tree,
                loaded_at_epoch,
                initial_selection_hint,
            } => {
                self.pending_pin_remove = None;
                self.set_data(snapshot, tree, loaded_at_epoch, initial_selection_hint);
            }
            Msg::SetTree(tree) => {
                self.pending_pin_remove = None;
                self.set_tree(tree);
            }
            Msg::SelectRow(id) => {
                self.pending_pin_remove = None;
                self.set_selection(*id);
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
            Msg::LeftViewportChanged { viewport_height } => {
                // The draw path dispatches this
                // before building the left panel's line list so
                // scroll reconciliation lives in the reducer, not
                // in `draw`. Each visible row contributes exactly
                // one primary line, so the selected row's line
                // index equals its position in the visible
                // projection. No selection → no reconciliation
                // (empty tree keeps scroll at 0).
                let Some(selected) = self.selection.clone() else {
                    return effects;
                };
                let Some(line_idx) = self.visible_rows().iter().position(|r| r.id == selected)
                else {
                    return effects;
                };
                self.adjust_left_scroll(line_idx, viewport_height);
            }
            Msg::ExplorerViewportChanged {
                cursor_first_row,
                cursor_last_row,
                viewport_height,
            } => {
                // Draw computes the post-wrap
                // cursor row span (Paragraph::wrap output isn't
                // pure over App state — it depends on pane width
                // and font metrics), and this Msg carries the
                // measured span into the reducer's
                // adjust_explorer_scroll math.
                self.adjust_explorer_scroll(cursor_first_row, cursor_last_row, viewport_height);
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
            Msg::InFlightStart { kind, label } => {
                let op = InFlightOp {
                    kind: kind.clone(),
                    label,
                    started_at: Instant::now(),
                };
                if let Some(existing) = self.in_flight_ops.iter_mut().find(|o| o.kind == kind) {
                    *existing = op;
                } else {
                    self.in_flight_ops.push(op);
                }
            }
            Msg::InFlightFinish(kind) => {
                self.in_flight_ops.retain(|o| o.kind != kind);
            }
            Msg::Viewer(vmsg) => {
                use crate::viewer::input::{ViewerEffect, reduce};
                if !matches!(self.modal_stack.last(), Some(crate::tui::Modal::Viewer(_))) {
                    return effects;
                }
                let Some(crate::tui::Modal::Viewer(state)) = self.modal_stack.pop() else {
                    unreachable!("checked above");
                };
                let (next, effect) = reduce(state, vmsg);
                match effect {
                    ViewerEffect::Close => {
                        self.status_message = Some("viewer closed".to_string());
                    }
                    ViewerEffect::None => {
                        self.modal_stack.push(crate::tui::Modal::Viewer(next));
                    }
                }
            }
            Msg::AttachSelected => {
                use crate::tui::effect::ExecSpec;
                match crate::tui::actions::resolve_attach_target(self) {
                    Ok(target) => effects.push(Effect::Exec(ExecSpec::AttachMux(target))),
                    Err(reason) => effects.push(Effect::Toast(
                        crate::tui::actions::attach_disabled_reason(&reason),
                    )),
                }
            }
            Msg::ResumeSelected => {
                use crate::tui::effect::ExecSpec;
                let Some(selection) = self.selection.clone() else {
                    effects.push(Effect::Toast("resume: nothing selected".to_string()));
                    return effects;
                };
                let session_id =
                    if let RowId::AgentSession(crate::model::NodeId::AgentSession(id)) = &selection
                    {
                        id.clone()
                    } else {
                        effects.push(Effect::Toast("resume: select an agent session".to_string()));
                        return effects;
                    };
                let target = crate::tui::resume::resolve_resume_target(&session_id);
                match &target {
                    crate::tui::resume::ResumeTarget::Launch { .. } => {
                        effects.push(Effect::Exec(ExecSpec::Resume(target)));
                    }
                    _ => effects.push(Effect::Toast(crate::tui::resume::resume_disabled_reason(
                        &target,
                    ))),
                }
            }
            Msg::ViewSelected => {
                use crate::tui::effect::ExecSpec;
                match crate::tui::actions::resolve_view_session(self) {
                    Ok(session_id) => {
                        effects.push(Effect::Exec(ExecSpec::ViewSession(session_id)));
                    }
                    Err(reason) => effects.push(Effect::Toast(
                        crate::tui::viewer::viewer_disabled_reason(&reason),
                    )),
                }
            }
            Msg::LaunchSelectedPin => {
                use crate::tui::effect::ExecSpec;
                match crate::tui::actions::resolve_launch_pin(self) {
                    Ok((pin_id, attach_target)) => {
                        effects.push(Effect::Exec(ExecSpec::LaunchPin {
                            pin_id,
                            attach_target,
                        }));
                    }
                    Err(reason) => effects.push(Effect::Toast(
                        crate::tui::actions::pin_launch_disabled_reason(&reason),
                    )),
                }
            }
            Msg::LaunchPinById(pin_id) => {
                use crate::tui::effect::ExecSpec;
                let attach_target =
                    crate::tui::actions::pin_launch_target_from_snapshot(self, &pin_id);
                effects.push(Effect::Exec(ExecSpec::LaunchPin {
                    pin_id,
                    attach_target,
                }));
            }
            Msg::PinRemove(request) => {
                effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::PinRemove(
                    request,
                )));
            }
            Msg::PinBind(request) => {
                if self.handle.is_none() {
                    effects.push(Effect::Toast(
                        "pin bind failed: no graph loaded yet".to_string(),
                    ));
                } else {
                    effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::PinBind(
                        request,
                    )));
                }
            }
            Msg::PinCreate(request) => {
                effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::PinCreate(
                    request,
                )));
            }
            Msg::PinEdit(request) => {
                effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::PinEdit(
                    request,
                )));
            }
            Msg::SwitchView(view) => {
                let before = self.active_view;
                self.switch_to_view(view);
                self.rebuild_tree_in_place();
                if self.active_view != before {
                    effects.push(Effect::Persist);
                }
            }
            Msg::SetGrouping(g) => {
                self.grouping = g;
                if matches!(g, super::Grouping::Sessions(_)) {
                    self.force_recency_for_flat_sessions();
                }
                self.rebuild_tree_in_place();
                effects.push(Effect::Persist);
            }
            Msg::SetFilter(filter) => {
                self.filter = filter;
                self.rebuild_tree_in_place();
                effects.push(Effect::Persist);
            }
            Msg::SetSort(sort) => {
                let sort = if matches!(
                    self.grouping,
                    super::Grouping::Sessions(super::SessionsGrouping::None)
                ) {
                    super::Sort::Recency
                } else {
                    sort
                };
                self.sort = sort;
                self.rebuild_tree_in_place();
                effects.push(Effect::Persist);
            }
            Msg::SetMuxRecency(basis) => {
                // Selecting a recency basis implies recency ordering —
                // otherwise the choice would silently do nothing under
                // Hierarchy sort. Nudge sort to Recency so the change is
                // immediately visible in the mux tree.
                self.mux_recency = basis;
                self.sort = super::Sort::Recency;
                self.rebuild_tree_in_place();
                effects.push(Effect::Persist);
            }
            Msg::CommitRename(value) => effects.push(self.rename_commit_effect(&value)),
            Msg::CommitWorktreeCreate { repo_root, branch } => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::WorktreeCreate { repo_root, branch },
                ));
            }
            Msg::CommitWorktreeRemove {
                repo_root,
                branch,
                force,
            } => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::WorktreeRemove {
                        repo_root,
                        branch,
                        force,
                    },
                ));
            }
            Msg::CommitWorktreeMerge {
                worktree_root,
                target,
            } => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::WorktreeMerge {
                        worktree_root,
                        target,
                    },
                ));
            }
            Msg::CommitWorktreeCloseDown {
                repo_root,
                branch,
                discard,
            } => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::WorktreeCloseDown {
                        repo_root,
                        branch,
                        discard,
                    },
                ));
            }
            Msg::CommitWorktreePrune { repo_root } => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::WorktreePrune { repo_root },
                ));
            }
            Msg::CommitMuxNew { name, cwd } => {
                effects.push(Effect::Exec(crate::tui::effect::ExecSpec::MuxNew {
                    name,
                    cwd,
                }));
            }
            Msg::OpenNewMuxForm => {
                let seeded_cwd = crate::tui::runtime::derive_mux_form_cwd(self)
                    .unwrap_or_else(|| std::env::var("HOME").unwrap_or_else(|_| "/".to_string()));
                self.open_new_mux_form(crate::tui::widgets::new_mux::NewMuxFormState::new(
                    String::new(),
                    seeded_cwd,
                ));
            }
            Msg::OpenMuxLaunchForm => {
                let seeded_cwd = crate::tui::runtime::derive_mux_form_cwd(self)
                    .unwrap_or_else(|| std::env::var("HOME").unwrap_or_else(|_| "/".to_string()));
                let seeded_mux_name =
                    crate::tui::runtime::derive_mux_launch_name(self).unwrap_or_default();
                let known_harness_keys = crate::tui::runtime::known_harness_keys();
                let known_mux_names = crate::tui::runtime::known_live_mux_names(self);
                let default_harness = known_harness_keys.first().cloned().unwrap_or_default();
                self.open_mux_launch_form(
                    crate::tui::widgets::mux_launch::MuxLaunchFormState::new(
                        default_harness,
                        seeded_cwd,
                        seeded_mux_name,
                        known_harness_keys,
                        known_mux_names,
                    ),
                );
            }
            Msg::CommitMuxLaunch(request) => {
                effects.push(Effect::Exec(crate::tui::effect::ExecSpec::MuxLaunch {
                    request,
                }));
            }
        }
        effects
    }

    /// The effect that commits the rename overlay's value for the
    /// current selection: an alias, pin, or mux rename, or a toast
    /// explaining why nothing can be renamed.
    fn rename_commit_effect(&self, value: &str) -> super::Effect {
        use super::Effect;
        match self.selection.clone() {
            Some(RowId::AgentSession(NodeId::AgentSession(id))) => {
                let trimmed = value.trim().to_string();
                let new_display_name = (!trimmed.is_empty()).then_some(trimmed);
                Effect::WriteStore(crate::tui::effect::StoreOp::CommitAliasRename {
                    session_id: id,
                    new_display_name,
                })
            }
            Some(RowId::Pin { pin_id }) => {
                let display = value.trim();
                if display.is_empty() {
                    return Effect::Toast("pin rename: display name cannot be empty".to_string());
                }
                let Some(target) = self.pins_context().pin_target else {
                    return Effect::Toast(format!(
                        "pin rename: no editable pin `{pin_id}` in current selection"
                    ));
                };
                Effect::WriteStore(crate::tui::effect::StoreOp::PinEdit(
                    crate::tui::widgets::pins::PinEditRequest {
                        original_id: target.id.clone(),
                        id: target.id,
                        display_name: display.to_string(),
                        harness: target.harness,
                        cwd: target.cwd,
                        mux_name: target.mux_name,
                        mux_socket: target.mux_socket,
                        launch_argv: target.launch_argv,
                        store_path: target.store_path,
                    },
                ))
            }
            Some(RowId::MuxSession(NodeId::MuxSession(mux_id))) => {
                let new_name = value.trim().to_string();
                if new_name.is_empty() {
                    return Effect::Toast("mux rename: name cannot be empty".to_string());
                }
                Effect::WriteStore(crate::tui::effect::StoreOp::CommitMuxRename {
                    mux_id,
                    new_name,
                })
            }
            _ => Effect::Toast("rename: lost selection before commit".to_string()),
        }
    }
}

/// Pick the detail-pane target for a placeholder pin row given the
/// active view. View-aligned: the sessions view upgrades to the
/// pin's `last_session` when one is recorded *and* the corresponding
/// agent-session node is in the snapshot; the mux view upgrades to
/// the bound / stale-mux target when the pin has one. Otherwise the
/// pin node itself is the detail target so the operator sees a sparse
/// "no live entity" surface rather than a mismatched session/mux view.
fn placeholder_detail_target(snapshot: &GraphSnapshot, pin_id: &str, view: View) -> NodeId {
    let pin_node = || NodeId::Pin(PinId::new(pin_id.to_string()));
    let Some(pin) = snapshot.pins.iter().find(|p| p.id == pin_id) else {
        return pin_node();
    };
    match view {
        View::Mux => match &pin.binding {
            Some(PinBinding::Bound { mux, .. } | PinBinding::StaleMux { mux }) => {
                NodeId::MuxSession(mux.clone())
            }
            _ => pin_node(),
        },
        View::Sessions => {
            if let Some(PinBinding::Bound { session, .. }) = &pin.binding {
                return NodeId::AgentSession(session.clone());
            }
            let last_session_key = snapshot.diagnostics.iter().find_map(|d| match d {
                Diagnostic::PinUnbound {
                    pin_id: id,
                    last_session: Some(last),
                    ..
                } if id == pin_id => Some(last.session_id.as_str()),
                _ => None,
            });
            if let Some(session_key) = last_session_key {
                let upgraded = snapshot.nodes.iter().find_map(|n| match n {
                    GraphNode::AgentSession(s)
                        if s.id.harness_key == pin.harness && s.id.session_key == session_key =>
                    {
                        Some(NodeId::AgentSession(s.id.clone()))
                    }
                    _ => None,
                });
                if let Some(upgraded) = upgraded {
                    return upgraded;
                }
            }
            pin_node()
        }
        View::Union | View::Prs | View::Forks => pin_node(),
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
#[path = "app_tests.rs"]
mod tests;
