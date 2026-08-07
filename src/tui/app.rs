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
    /// In-flight async operations the status bar surfaces as
    /// animated spinner chips (H-WIDG-007). Ordered `Vec` so the
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
    /// visible. H-TUI-005 wave 1 dropped the `Cell` interior
    /// mutability: draw now takes `&mut App` and mutates the
    /// field directly; wave 2 will move the reconciliation into
    /// the reducer via a `Msg::LeftViewportChanged` per ADR 0085
    /// contract 5.
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
    /// removal a confirmation step without introducing a full modal
    /// before the H-PIN-023 edit/remove flow lands.
    pending_pin_remove: Option<String>,
    /// Open overlays as a stack (ADR 0085 contract 3). Input routes
    /// to the top entry first, `draw` renders bottom-to-top, and
    /// commit / close outcomes pop the top. Grows one variant per
    /// overlay migration wave — see [`crate::tui::Modal`]. The
    /// remaining `Option<...>` fields below still host overlays
    /// that haven't migrated yet; each wave deletes one field and
    /// moves the state to a `Modal` variant.
    modal_stack: Vec<crate::tui::Modal>,
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
    /// Mux-view recency basis (H-MUX-SORT-001). Selects which epoch
    /// `Sort::Recency` orders the mux tree by (activity / created /
    /// last-attached). Mux-scoped: no other view reads it. Seeded from
    /// [`RunConfig::default_mux_recency`].
    mux_recency: super::MuxRecency,
    /// Active row filter (ADR 0031, F8-003). Mirrors the active
    /// view's slot in `view_states` so callers don't pay a map
    /// lookup per read. Kept in sync via `switch_to_view` /
    /// `Msg::SetFilter`.
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

/// Discriminant for an async operation the TUI wants to signal to
/// the operator (H-WIDG-007 substrate). Grows with each new async
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
    /// Projection-only row-tree replacement (ADR 0085 contract 4).
    /// The runtime dispatches this after a view / grouping / filter
    /// / sort change so the tree re-derives from the held snapshot
    /// without touching discovery. Snapshot and `loaded_at_epoch`
    /// are untouched — the data isn't fresher, only re-projected —
    /// and the `initial_selection_hint` path is skipped because
    /// `SetTree` never fires before the first `SetData`.
    SetTree(RowTree),
    /// Set the left-panel selection to a specific row id
    /// (H-TUI-006). Used by the search overlay's Confirm outcome so
    /// its selection change flows through the reducer instead of a
    /// direct `App::set_selection` call inside a specialized runtime
    /// handler.
    SelectRow(Box<RowId>),
    /// Left panel: move selection down/up one visible row.
    NavDown,
    NavUp,
    /// Left panel: page through visible rows. The runtime passes
    /// the rendered viewport height so the reducer can decide how
    /// many rows constitute one page. Pass 1 if unknown.
    PageDown(u16),
    PageUp(u16),
    /// Left panel: post-layout viewport dimensions
    /// (H-TUI-005 wave 2). The draw path dispatches this before
    /// building the left panel's line list so the reducer can
    /// reconcile `left_scroll` from the current selection and
    /// viewport height instead of the renderer poking a `Cell`
    /// mid-frame. Carries the tree pane's inner height (rows).
    /// The reducer computes the selected row's line index from
    /// the visible-row projection — each visible row contributes
    /// exactly one primary line, so the visible-row position is
    /// the line index.
    LeftViewportChanged {
        viewport_height: u16,
    },
    /// Explorer (right panel): post-layout cursor row span and
    /// viewport dimensions (H-TUI-005 wave 2). The draw path
    /// dispatches this after computing the post-wrap row span of
    /// the explorer cursor so the reducer can reconcile
    /// `explorer_scroll`. Unlike the left panel, cursor row
    /// positions depend on `Paragraph::wrap` output for the
    /// widget-rendered lines, so the draw path measures and the
    /// reducer records; the split keeps scroll reconciliation
    /// itself out of the render pipeline.
    ExplorerViewportChanged {
        cursor_first_row: usize,
        cursor_last_row: usize,
        viewport_height: u16,
    },
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
    /// Start tracking a new in-flight async operation (H-WIDG-007).
    /// The reducer stamps `Instant::now()` and stores the op; the
    /// status bar renders one animated spinner chip per active op.
    /// Idempotent by `InFlightKind`: starting the same kind twice
    /// replaces the prior record's `started_at` and label.
    InFlightStart {
        kind: InFlightKind,
        label: String,
    },
    /// Mark an in-flight async operation complete (H-WIDG-007).
    /// Removes the matching kind from the tracker; a no-op if no
    /// op with that kind is currently in flight.
    InFlightFinish(InFlightKind),
    /// Nested-reducer entry point for the transcript viewer
    /// (ADR 0085 contract 3, H-TUI-003 wave 7). The reducer arm
    /// pops the top viewer state, runs it through
    /// [`crate::viewer::input::reduce`], and pushes the new state
    /// back on `ViewerEffect::None` or leaves the stack popped
    /// on `ViewerEffect::Close`. No-op when the top of the modal
    /// stack isn't the viewer.
    Viewer(crate::viewer::input::ViewerMsg),
    /// Attach to the currently selected row's mux session
    /// (ADR 0085 contract 2). The reducer resolves the target via
    /// `resolve_attach_target(&self)` and emits either
    /// `Effect::Exec(ExecSpec::AttachMux(target))` when attachable
    /// or `Effect::Toast(reason)` when disabled. All the terminal
    /// / `tmux` I/O lives in the executor.
    AttachSelected,
    /// Resume the currently selected agent session (ADR 0085
    /// contract 2). The reducer resolves the resume target and
    /// emits either `Effect::Exec(ExecSpec::Resume(target))` when
    /// launchable or `Effect::Toast(reason)` when disabled.
    ResumeSelected,
    /// Open the transcript viewer for the current selection
    /// (ADR 0085 contract 2). The reducer resolves the session id
    /// via `resolve_view_session` and emits either
    /// `Effect::Exec(ExecSpec::ViewSession(id))` or
    /// `Effect::Toast(reason)`. The native-vs-external branching
    /// (and the accompanying filesystem read) happens in the
    /// executor, not here.
    ViewSelected,
    /// Launch the pin backing the current selection (ADR 0085
    /// contract 2). The reducer resolves the pin id + optional
    /// attach target via `resolve_launch_pin` and emits either
    /// `Effect::Exec(ExecSpec::LaunchPin { pin_id, attach_target })`
    /// or `Effect::Toast(reason)`.
    LaunchSelectedPin,
    /// Launch a specific pin by id (ADR 0057 / ADR 0058). Used by
    /// the Pins overlay's launch entry, which already knows the
    /// pin id and doesn't need selection-based resolution. The
    /// reducer looks up the optional attach target from the held
    /// snapshot and emits
    /// `Effect::Exec(ExecSpec::LaunchPin { pin_id, attach_target })`.
    LaunchPinById(String),
    /// Remove a pin declaration from its TOML store (ADR 0057).
    /// Carries the already-resolved [`PinRemoveRequest`]; the
    /// reducer emits `Effect::WriteStore(StoreOp::PinRemove(...))`
    /// and the executor performs the write.
    PinRemove(crate::tui::widgets::pins::PinRemoveRequest),
    /// Write a pin-binding declaration linking a pin to an existing
    /// agent session (ADR 0057 / ADR 0058). The reducer checks that
    /// a graph snapshot is loaded (needed for the target lookup at
    /// executor time) and emits either
    /// `Effect::WriteStore(StoreOp::PinBind(...))` or
    /// `Effect::Toast(reason)`.
    PinBind(crate::tui::widgets::pins::PinBindRequest),
    /// Write a new pin entry (ADR 0057). When the request carries
    /// an `adopt_source_mux_name`, the executor chains a tmux
    /// rename after the write to promote the adopted mux to the
    /// pin's declared name.
    PinCreate(crate::tui::widgets::pins::PinCreateRequest),
    /// Update an existing pin entry (ADR 0057).
    PinEdit(crate::tui::widgets::pins::PinEditRequest),
    /// Confirm the rename overlay's typed value (ADR 0029).
    /// Reducer resolves the selected row and emits either a pin
    /// edit, an agent-session alias rename, or a "lost selection"
    /// toast. The alias-rename branch also chains an optional
    /// native mux rename inside the executor per lockstep.
    CommitRename(String),
    /// Commit the worktree menu's "New worktree" branch input
    /// (H-WT-004b). Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreeCreate)`.
    CommitWorktreeCreate {
        repo_root: String,
        branch: String,
    },
    /// Commit the worktree menu's "Remove worktree" confirm
    /// (H-WT-004b). Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreeRemove)`.
    CommitWorktreeRemove {
        repo_root: String,
        branch: String,
        force: bool,
    },
    /// Commit the worktree menu's "Merge back & close" confirm
    /// (H-WT-005). Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreeMerge)`.
    CommitWorktreeMerge {
        worktree_root: String,
        target: Option<String>,
    },
    /// Commit the worktree menu's "Close down stream" choice
    /// (H-WT-006). Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreeCloseDown)`. `discard`
    /// drops the branch; otherwise it is merged back first.
    CommitWorktreeCloseDown {
        repo_root: String,
        branch: String,
        discard: bool,
    },
    /// Commit the worktree menu's "Prune merged worktrees" confirm
    /// (H-WT-008). Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreePrune)`.
    CommitWorktreePrune {
        repo_root: String,
    },
    /// Switch the active row-tree view (ADR 0031). Reducer saves
    /// the current view's per-view slot, loads the target's slot
    /// (or fresh defaults on first visit), and re-derives the row
    /// tree from the held snapshot — projection-only, never
    /// triggers discovery (ADR 0085 contract 4).
    SwitchView(View),
    /// Update the active view's grouping (ADR 0031). Reducer
    /// mutates the projection state and re-derives the tree.
    SetGrouping(super::Grouping),
    /// Update the active row filter (ADR 0031, F8-003). Reducer
    /// mutates the projection state and re-derives the tree.
    SetFilter(crate::filter::RowFilter),
    /// Update the global sort (ADR 0031). Reducer mutates the
    /// projection state and re-derives the tree; the flat-
    /// sessions grouping forces recency regardless of the
    /// requested value.
    SetSort(super::Sort),
    /// Update the mux-view recency basis (H-MUX-SORT-001). Reducer
    /// stores the basis, forces `Sort::Recency` so the choice takes
    /// effect, and re-derives the tree.
    SetMuxRecency(super::MuxRecency),
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

    /// Active rename-overlay state, if any. Lives on the modal
    /// stack (ADR 0085 contract 3); this accessor peeks the top
    /// entry.
    pub fn rename_overlay(&self) -> Option<&crate::tui::widgets::input::TextInputState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Rename(state) => Some(state.inner()),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn rename_overlay_mut(&mut self) -> Option<&mut crate::tui::RenameOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Rename(state) => Some(state),
            _ => None,
        }
    }

    /// Push a rename overlay onto the modal stack. Caller
    /// pre-populates the input with the current alias, harness
    /// title, or empty string per ADR 0030. H-TUI-006 wraps the
    /// raw text-input state in a [`RenameOverlayState`] so the
    /// overlay's Confirm(String) maps to Msg::CommitRename via
    /// the uniform Overlay trait.
    pub fn open_rename_overlay(&mut self, state: crate::tui::widgets::input::TextInputState) {
        self.modal_stack.push(crate::tui::Modal::Rename(
            crate::tui::RenameOverlayState::new(state),
        ));
    }

    /// Pop the rename overlay if it's on top; no-op otherwise.
    pub fn close_rename_overlay(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Rename(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active worktree action menu (H-WT-004b), if on top of the stack.
    pub fn worktree_menu(&self) -> Option<&crate::tui::widgets::worktree_menu::WorktreeMenuState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::WorktreeMenu(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn worktree_menu_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::worktree_menu::WorktreeMenuState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::WorktreeMenu(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    /// Push a worktree action menu onto the modal stack (H-WT-004b).
    pub fn open_worktree_menu(
        &mut self,
        state: crate::tui::widgets::worktree_menu::WorktreeMenuState,
    ) {
        self.modal_stack
            .push(crate::tui::Modal::WorktreeMenu(Box::new(state)));
    }

    /// Pop the worktree menu if it's on top; no-op otherwise.
    pub fn close_worktree_menu(&mut self) {
        if matches!(
            self.modal_stack.last(),
            Some(crate::tui::Modal::WorktreeMenu(_))
        ) {
            self.modal_stack.pop();
        }
    }

    pub fn pending_pin_remove(&self) -> Option<&str> {
        self.pending_pin_remove.as_deref()
    }

    pub fn set_pending_pin_remove(&mut self, pin_id: Option<String>) {
        self.pending_pin_remove = pin_id;
    }

    /// Active controls-overlay state (ADR 0031, F8-004), if any.
    /// Lives on the modal stack (ADR 0085 contract 3); this
    /// accessor peeks the top entry.
    pub fn controls_overlay(&self) -> Option<&crate::tui::widgets::controls::ControlsOverlayState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Controls(state) => Some(state),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn controls_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::controls::ControlsOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Controls(state) => Some(state),
            _ => None,
        }
    }

    /// Push a fresh controls overlay onto the modal stack, cursor
    /// on the active view row.
    pub fn open_controls_overlay(&mut self) {
        let ctx = self.controls_context();
        self.modal_stack.push(crate::tui::Modal::Controls(
            crate::tui::widgets::controls::ControlsOverlayState::new(&ctx),
        ));
    }

    /// Pop the controls overlay if it's on top; no-op otherwise.
    pub fn close_controls_overlay(&mut self) {
        if matches!(
            self.modal_stack.last(),
            Some(crate::tui::Modal::Controls(_))
        ) {
            self.modal_stack.pop();
        }
    }

    /// Active pins-overlay state (ADR 0057), if any. Lives on the
    /// modal stack (ADR 0085 contract 3); this accessor peeks the
    /// top entry.
    pub fn pins_overlay(&self) -> Option<&crate::tui::widgets::pins::PinsOverlayState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Pins(state) => Some(state),
            _ => None,
        }
    }

    pub fn pins_overlay_mut(&mut self) -> Option<&mut crate::tui::widgets::pins::PinsOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Pins(state) => Some(state),
            _ => None,
        }
    }

    /// Push a fresh pins overlay onto the modal stack at the top
    /// of the action list.
    pub fn open_pins_overlay(&mut self) {
        self.modal_stack.push(crate::tui::Modal::Pins(
            crate::tui::widgets::pins::PinsOverlayState::new(),
        ));
    }

    /// Push a pre-configured pins overlay state — used by direct
    /// shortcuts (`N`/`B`/`A`/`b`) that skip the menu and open a
    /// sub-editor directly.
    pub fn set_pins_overlay(&mut self, state: crate::tui::widgets::pins::PinsOverlayState) {
        self.modal_stack.push(crate::tui::Modal::Pins(state));
    }

    /// Pop the pins overlay if it's on top; no-op otherwise.
    pub fn close_pins_overlay(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Pins(_))) {
            self.modal_stack.pop();
        }
    }

    /// Snapshot of the live pin state the pins overlay renders against.
    pub fn pins_context(&self) -> crate::tui::widgets::pins::PinsContext {
        crate::tui::widgets::pins::PinsContext {
            pin_create_defaults: self.pin_create_defaults(),
            pin_adopt_defaults: self.pin_adopt_defaults_if_available(),
            known_cwd_candidates: self.known_pin_cwd_candidates(),
            known_harness_keys: self.known_harness_keys().into_iter().collect(),
            known_mux_names: self.used_mux_names().into_iter().collect(),
            known_pin_ids: self.used_pin_ids().into_iter().collect(),
            known_pin_mux_names: self.used_pin_mux_names().into_iter().collect(),
            selected_pin_id: self.selected_pin_id(),
            pin_target: self.pin_mutation_target(),
            pin_bind_options: self.pin_bind_options(),
        }
    }

    /// Active `/` search overlay (T8-017), if any. Lives on the
    /// modal stack (ADR 0085 contract 3); this accessor peeks the
    /// top entry.
    pub fn search_overlay(&self) -> Option<&crate::tui::widgets::search::SearchOverlayState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Search(state) => Some(state),
            _ => None,
        }
    }

    pub fn search_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::search::SearchOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Search(state) => Some(state),
            _ => None,
        }
    }

    /// Push a fresh search overlay onto the modal stack.
    pub fn open_search_overlay(&mut self) {
        self.modal_stack.push(crate::tui::Modal::Search(
            crate::tui::widgets::search::SearchOverlayState::new(),
        ));
    }

    /// Pop the search overlay if it's on top; no-op otherwise.
    pub fn close_search_overlay(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Search(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active `?` help overlay (F8-011), if any. Lives on the modal
    /// stack (ADR 0085 contract 3); this accessor peeks the top
    /// entry.
    pub fn help_overlay(&self) -> Option<&crate::tui::widgets::help::HelpOverlayState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Help(state) => Some(state),
            _ => None,
        }
    }

    pub fn help_overlay_mut(&mut self) -> Option<&mut crate::tui::widgets::help::HelpOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Help(state) => Some(state),
            _ => None,
        }
    }

    /// Push a fresh help overlay onto the modal stack.
    pub fn open_help_overlay(&mut self) {
        self.modal_stack.push(crate::tui::Modal::Help(
            crate::tui::widgets::help::HelpOverlayState::new(),
        ));
    }

    /// Pop the help overlay if it's on top; no-op otherwise.
    pub fn close_help_overlay(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Help(_))) {
            self.modal_stack.pop();
        }
    }

    /// The modal stack (ADR 0085 contract 3). Reserved for
    /// generic stack-operating code (draw sweep, generic
    /// input-routing helper); overlay-specific consumers use the
    /// per-overlay accessors like `help_overlay()`.
    #[cfg(test)]
    pub(crate) fn modal_stack(&self) -> &[crate::tui::Modal] {
        &self.modal_stack
    }

    /// Active `o` full-value modal (T8-030), if any. Lives on the
    /// modal stack (ADR 0085 contract 3); this accessor peeks the
    /// top entry.
    pub fn value_modal(&self) -> Option<&crate::tui::widgets::value_modal::ValueModalState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::ValueModal(state) => Some(state),
            _ => None,
        }
    }

    pub fn value_modal_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::value_modal::ValueModalState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::ValueModal(state) => Some(state),
            _ => None,
        }
    }

    /// Pop the value modal if it's on top; no-op otherwise.
    pub fn close_value_modal(&mut self) {
        if matches!(
            self.modal_stack.last(),
            Some(crate::tui::Modal::ValueModal(_))
        ) {
            self.modal_stack.pop();
        }
    }

    /// Read-only access to the active transcript viewer modal
    /// (H-VIEWER-NATIVE-008). Lives on the modal stack
    /// (ADR 0085 contract 3); this accessor peeks the top entry.
    pub fn viewer_modal(&self) -> Option<&crate::viewer::state::ViewerState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Viewer(state) => Some(state),
            _ => None,
        }
    }

    /// Mutable access for the draw path (the widget writes back
    /// viewport_height + total_lines metrics during render).
    pub fn viewer_modal_mut(&mut self) -> Option<&mut crate::viewer::state::ViewerState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Viewer(state) => Some(state),
            _ => None,
        }
    }

    /// Push a viewer modal onto the stack. Callers construct the
    /// initial state via `viewer_bridge::build_viewer_state` and
    /// pass ownership here.
    pub fn open_viewer_modal(&mut self, state: crate::viewer::state::ViewerState) {
        self.modal_stack.push(crate::tui::Modal::Viewer(state));
    }

    /// Pop the viewer modal if it's on top; no-op otherwise.
    pub fn close_viewer_modal(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Viewer(_))) {
            self.modal_stack.pop();
        }
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
        self.last_visible_index = Some(idx);
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
            view: self.active_view,
            grouping: self.grouping,
            filter: &self.filter,
            sort: self.sort,
            mux_recency: self.mux_recency,
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
                    .map_or_else(|| session.session.session_key.clone(), str::to_string);
                let display = pin_create_default_name_candidate(&display);
                let id = pin_id_candidate(&display);
                let mux_name = self.unique_pin_mux_name(&id);
                PinCreateDefaults {
                    id,
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

    fn known_pin_cwd_candidates(&self) -> Vec<PathCandidate> {
        let mut candidates = Vec::new();
        let mut push = |path: &str, source: &str, rank: i32| {
            let path = path.trim();
            if !path.is_empty() {
                candidates.push(PathCandidate::new(path, source, rank));
            }
        };
        let defaults = self.pin_create_defaults();
        push(&defaults.cwd, "selected", 1_000);
        if let Some(adopt) = self.pin_adopt_defaults_if_available() {
            push(&adopt.cwd, "adopt", 900);
        }
        if let Some(database) = self.database.as_ref() {
            for pin in &database.snapshot().pins {
                push(&pin.cwd, "pin", 500);
            }
            for node in &database.snapshot().nodes {
                match node {
                    crate::model::GraphNode::AgentSession(node) => {
                        if let Some(cwd) = node.cwd.as_deref() {
                            push(cwd, "agent", 450);
                        }
                    }
                    crate::model::GraphNode::MuxSession(node) => {
                        if let Some(cwd) = node.cwd.as_deref() {
                            push(cwd, "mux", 440);
                        }
                        if let Some(cwd) = node.active_pane_current_path.as_deref() {
                            push(cwd, "mux", 430);
                        }
                    }
                    crate::model::GraphNode::RuntimeProcess(node) => {
                        if let Some(cwd) = node.cwd.as_deref() {
                            push(cwd, "process", 400);
                        }
                    }
                    crate::model::GraphNode::Checkout(node) => {
                        push(&node.root, "checkout", 350);
                    }
                    crate::model::GraphNode::Repo(node) => {
                        let root = node
                            .common_dir
                            .strip_suffix("/.git")
                            .unwrap_or(&node.common_dir);
                        push(root, "repo", 320);
                    }
                    crate::model::GraphNode::Workspace(node) => {
                        push(&node.root, "workspace", 300);
                    }
                    crate::model::GraphNode::Pin(node) => {
                        push(&node.cwd, "pin", 500);
                    }
                    crate::model::GraphNode::Branch(_)
                    | crate::model::GraphNode::Fork(_)
                    | crate::model::GraphNode::ForgePr(_) => {}
                }
            }
        }
        candidates
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
        let mut keys: BTreeSet<String> = harness_options()
            .iter()
            .map(|key| key.to_string())
            .collect();
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

    /// Mux-view recency basis (H-MUX-SORT-001).
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
        }

        // Mux recency basis (H-MUX-SORT-001): restore the operator's
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

    /// Build a [`PersistedState`] snapshot of the current app state
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

    /// In-flight async operations tracked for status-bar spinner
    /// chips (H-WIDG-007). Empty when nothing is in flight.
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
        let clamped = offset.min(u16::MAX as usize) as u16;
        self.left_scroll = clamped;
        clamped
    }

    /// Pure getter for the current left-panel scroll offset. The
    /// reducer owns updates (via
    /// [`Msg::LeftViewportChanged`] per H-TUI-005 wave 2); the
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
        let clamped = offset.min(u16::MAX as usize) as u16;
        self.explorer_scroll = clamped;
        clamped
    }

    /// Pure getter for the current explorer scroll offset. The
    /// reducer owns updates (via
    /// [`Msg::ExplorerViewportChanged`] per H-TUI-005 wave 2);
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
        // The empty-stack Backspace arming only persists across
        // consecutive Backspace presses; any other message clears it
        // so the operator doesn't accidentally back out of the right
        // pane after an intervening action.
        if !matches!(msg, Msg::ExplorerBack) {
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
                // H-TUI-005 wave 2: the draw path dispatches this
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
                // H-TUI-005 wave 2: draw computes the post-wrap
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
                let session_id = match &selection {
                    RowId::AgentSession(crate::model::NodeId::AgentSession(id)) => id.clone(),
                    _ => {
                        effects.push(Effect::Toast("resume: select an agent session".to_string()));
                        return effects;
                    }
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
                if self.database.is_none() {
                    effects.push(Effect::Toast(
                        "pin bind failed: no graph database available".to_string(),
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
            Msg::CommitRename(value) => match self.selection.clone() {
                Some(RowId::AgentSession(crate::model::NodeId::AgentSession(id))) => {
                    let trimmed = value.trim().to_string();
                    let new_display_name = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    };
                    effects.push(Effect::WriteStore(
                        crate::tui::effect::StoreOp::CommitAliasRename {
                            session_id: id,
                            new_display_name,
                        },
                    ));
                }
                Some(RowId::Pin { pin_id }) => {
                    let display = value.trim();
                    if display.is_empty() {
                        effects.push(Effect::Toast(
                            "pin rename: display name cannot be empty".to_string(),
                        ));
                        return effects;
                    }
                    let Some(target) = self.pins_context().pin_target else {
                        effects.push(Effect::Toast(format!(
                            "pin rename: no editable pin `{pin_id}` in current selection"
                        )));
                        return effects;
                    };
                    effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::PinEdit(
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
                    )));
                }
                Some(RowId::MuxSession(crate::model::NodeId::MuxSession(mux_id))) => {
                    let new_name = value.trim().to_string();
                    if new_name.is_empty() {
                        effects.push(Effect::Toast(
                            "mux rename: name cannot be empty".to_string(),
                        ));
                        return effects;
                    }
                    effects.push(Effect::WriteStore(
                        crate::tui::effect::StoreOp::CommitMuxRename { mux_id, new_name },
                    ));
                }
                _ => effects.push(Effect::Toast(
                    "rename: lost selection before commit".to_string(),
                )),
            },
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
        }
        effects
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
    fn rebuild_tree_in_place(&mut self) {
        let Some(db) = self.database.as_ref() else {
            return;
        };
        let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
            db.snapshot(),
            self,
        ));
        self.set_tree(tree);
    }

    fn set_tree(&mut self, tree: RowTree) {
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

    fn move_selection_to(&mut self, index: usize) {
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
    fn current_visible_index(&self, visible: &[RowId], selection: &RowId) -> usize {
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
        let Some(database) = self.database.as_ref() else {
            self.explorer = None;
            return;
        };
        let snapshot = database.snapshot();
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

    fn recompute_explorer_for(
        &mut self,
        target: NodeId,
        home: Option<&std::path::Path>,
        strip_relationships: bool,
    ) {
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
            Some(mut view) => {
                if strip_relationships {
                    view.relationships.groups.clear();
                }
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
            Some(PinBinding::Bound { mux, .. }) | Some(PinBinding::StaleMux { mux }) => {
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
