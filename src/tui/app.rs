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

use tuikit::feedback::{Feedback, Toasts};
use tuikit::scroll::{Anchor, ScrollRegion};
use tuikit::tree::{RowState, TreeState};

use crate::model::{AgentSessionId, GraphSnapshot, MuxSessionId, NodeId, PinId};
use crate::tui::explorer::{ExplorerRow, GraphExplorer, GraphSource};
use crate::tui::preview::{PreviewContent, PreviewEntry, PreviewSource, PreviewStore};
use crate::tui::rows::{Row, RowId, RowKind, RowTree};
use crate::tui::{RunConfig, View};

mod content;
mod content_history;
mod controls;
mod explorer_nav;
mod listing;
mod msg;
mod overlays;
mod search_open;
mod tree;
mod viewport;

#[cfg(test)]
pub(crate) use content::MAX_DETAIL_SESSIONS;
pub use content::{
    ContentSearch, ContentStatus, DetailRequest, PaneScope, ScanRequest, grouping_description,
};
pub use listing::Listing;
pub use msg::Msg;
#[cfg(test)]
pub(crate) use search_open::OpenPlan;
pub use viewport::ViewportReport;

/// What the Preview zone is pointed at ([`App::preview_focus`]): the left
/// selection, and the explorer's node with its cursor's position and row.
type PreviewFocus = (
    Option<RowId>,
    Option<(
        NodeId,
        Option<usize>,
        Option<crate::tui::explorer::ExplorerRowKey>,
    )>,
);

/// Cheap-to-clone, reference-counted handle to the App's resolved
/// [`GraphSnapshot`]. Read consumers borrow it via [`Self::snapshot`].
#[derive(Clone)]
pub struct SnapshotHandle(
    Rc<crate::model::GraphSnapshot>,
    Rc<std::cell::OnceCell<std::collections::BTreeMap<NodeId, crate::tui::rows::PinMarker>>>,
);

impl SnapshotHandle {
    pub(crate) fn new(snapshot: crate::model::GraphSnapshot) -> Self {
        Self(Rc::new(snapshot), Rc::default())
    }

    #[cfg(test)]
    pub(crate) fn from_snapshot(snapshot: &crate::model::GraphSnapshot) -> Self {
        Self(Rc::new(snapshot.clone()), Rc::default())
    }

    /// The entities pin slots point at, live or gone, with the marker
    /// each wears; the native views mark their rows with it.
    pub(crate) fn pinned_nodes(
        &self,
    ) -> &std::collections::BTreeMap<NodeId, crate::tui::rows::PinMarker> {
        self.1
            .get_or_init(|| crate::tui::rows::pinned_node_markers(&self.0))
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
    /// What the operator did to the active view's tree: the selected
    /// row by id (`None` when the tree is empty) and the position it
    /// showed at, the expanded rows (group rows the reducer opens on
    /// first sight included), and the window the last frame drew, which
    /// the next frame starts from and page keys move by. A frame reports
    /// the window in a [`ViewportReport`], which [`App::keep_viewports`]
    /// stores. Saved per view in [`ViewStateSlot`].
    tree_state: TreeState<RowId>,
    /// The right panel's graph explorer (`tuikit::explorer` over
    /// [`crate::tui::explorer::Graph`]): the focused node's view, the
    /// cursor, the `Other` fold, Expanded Node Detail, the drill frames,
    /// the armed `Backspace`, and the window its zone drew. Recomputed
    /// whenever selection or snapshot changes; a drill survives the
    /// recompute while the selection is still [`Self::explorer_reflects`].
    /// `None` until the reducer has resolved a selection into a node
    /// view, and again whenever the selection names no node; the right
    /// pane shows its placeholder then. Reducer code lends it out with a
    /// source over the rest of the app ([`Self::with_explorer`]).
    explorer: Option<GraphExplorer>,
    /// The left-pane selection the right pane reflects: the row it was
    /// last opened or reloaded for, the row the left pane moved to
    /// after a drill, or the row a `Backspace` put back. While the
    /// selection stays on it, a refresh or a projection change reloads
    /// the node a drilled pane shows instead of opening the pane on the
    /// selection's node again.
    explorer_reflects: Option<RowId>,
    /// Which panel currently consumes navigation keys.
    focus: Focus,
    /// Whether the explorer's link rows render the trailing
    /// `provenance · confidence · state` meta line.
    /// Initialized from `RunConfig::show_edge_meta`; flipped at
    /// runtime by `Msg::ToggleEdgeMeta`.
    edge_meta_visible: bool,
    /// The right-panel Preview's scroll region (ADR 0181): the rows it
    /// has been scrolled from its anchor (the top, a transcript's newest
    /// rows, or the search match it opened on), as the last frame drew
    /// it.
    preview: ScrollRegion,
    /// Unix-epoch seconds at which the current snapshot was
    /// loaded. `None` before the first `SetData`. The renderer
    /// turns this into the header's `updated Ns ago` indicator.
    loaded_at_epoch: Option<i64>,
    /// Transient status-bar message, e.g. the "disabled because…"
    /// reason for a key that didn't apply to the current selection,
    /// drawn in its severity's color. Cleared on the next selection /
    /// focus change.
    status_message: Option<Feedback>,
    /// Outcomes of operations run from this TUI (ADR 0105). In memory
    /// only.
    messages: crate::tui::messages::MessageLog,
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
    /// Rows whose values predate the operator's last tmux hand-off
    /// (ADR 0108). Cleared when a refresh spawned after the hand-off
    /// lands.
    handoff: Option<PendingHandoff>,
    /// An action that changed state asked for a background refresh; the
    /// runtime takes the request on its next pass.
    refresh_requested: bool,
    /// The entity the last action acted on, to select once the refresh
    /// after it lands (it may have moved to another view).
    pending_follow: Option<NodeId>,
    /// Where the operator was before each jump to another view, newest
    /// last; Backspace on the left pane walks it back.
    nav_back: Vec<NavEntry>,
    /// The row that was selected when the action started: if it is still
    /// listed once the refresh lands, the operator stays on it.
    pending_follow_row: Option<RowId>,
    /// The content search the session lists are filtered by, held in
    /// memory only.
    content: Option<content::ContentSearch>,
    /// The applied search the `/` overlay's preview replaced, kept so
    /// closing the overlay can bring it back.
    content_stash: Option<content::ContentSearch>,
    /// The content searches the Search view remembers (ADR 0149).
    search_history: content_history::SearchHistory,
    /// Numbers each search started, so its remembered copy can be found.
    content_serial: u64,
    /// What the `/` overlay was last set to, for the next time it opens.
    search_memory: Option<content::SearchMemory>,
    /// A scan the reducer wants started; the runtime takes it.
    scan_request: Option<content::ScanRequest>,
    detail_request: Option<content::DetailRequest>,
    /// Search-view rows already opened once, so a rebuild opens new
    /// buckets and sessions but does not undo a collapse.
    search_auto_expanded: BTreeSet<RowId>,
    /// How many times the tree was rebuilt in place, for tests that
    /// check a change rebuilds only when it must.
    #[cfg(test)]
    pub(super) tree_rebuilds: usize,
    /// Source of scan generations, so results of a scan the operator
    /// replaced or cleared are recognized and dropped.
    content_generation: u64,
    /// What an undo asked to select once its refresh lands.
    pending_restore_focus: Option<crate::tui::undo::RestoreFocus>,
    /// Operations the undo key can still reverse.
    undo: crate::tui::undo::UndoStack,
    /// Loaded previews keyed by source: pane captures and
    /// transcripts (ADR 0112 §5). The renderer reads this for the
    /// right-panel Preview zone.
    preview_store: PreviewStore,
    /// Open overlays as a stack (ADR 0085 contract 3, ADR 0161). Keys
    /// go to the top entry, every entry is drawn bottom to top, and a
    /// close or a commit closes the top. See [`crate::tui::modal`].
    modal_stack: crate::tui::modal::ModalStack,
    /// The entity the open rename prompt renames, fixed when it opened
    /// so a later focus change can't redirect the commit.
    rename_node: Option<NodeId>,
    /// The toast on screen, if any. A new toast replaces the one
    /// showing, and each goes when its severity's lifetime is up; the
    /// runtime calls [`Self::prepare_toast_for_render`] before each draw
    /// to retire an expired one. Non-blocking: input continues to flow
    /// to the underlying view.
    toasts: Toasts,
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
    /// How the preview pane fits a captured mux pane (ADR 0106).
    /// Seeded from [`RunConfig::default_preview_wrap`].
    preview_wrap: super::WrapMode,
    /// What the Preview shows for agent sessions in tmux (ADR 0112).
    /// Seeded from [`RunConfig::default_session_preview`].
    session_preview: super::SessionPreview,
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
/// grouping, and the tree's state (expanded-row set, selection, and
/// left-panel window). Constructed lazily — view_states is empty at
/// App::new and slots fill in as the operator visits each view (or
/// saves the currently-active view on the first switch away).
#[derive(Debug, Clone)]
pub(crate) struct ViewStateSlot {
    pub filter: crate::filter::RowFilter,
    pub grouping: super::Grouping,
    pub tree: TreeState<RowId>,
}

impl ViewStateSlot {
    fn defaults_for(view: View) -> Self {
        Self {
            filter: crate::filter::RowFilter::default(),
            grouping: super::Grouping::default_for(view),
            tree: TreeState::new(),
        }
    }
}

/// Which panel currently consumes navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Left,
    Right,
}

/// A place Backspace can return to: the view and the row that was
/// selected in it before a jump elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NavEntry {
    pub(crate) view: View,
    pub(crate) selection: Option<RowId>,
}

/// How many jumps Backspace remembers.
const NAV_BACK_LIMIT: usize = 20;

/// Discriminant for an async operation the TUI wants to signal to
/// the operator. Grows with each new async
/// surface — forge-metadata fetches, transcript loads, agent-deck
/// queries. The reducer stores at most one `InFlightOp` per kind, so
/// starting a new op with the same kind replaces any prior in-flight
/// record (used e.g. when a periodic discovery worker spawns while a
/// prior one is still in flight — the timer path already waits for
/// idle workers, but the model here is intentionally
/// idempotent).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InFlightKind {
    /// A discovery worker is scanning the local filesystem + forge +
    /// mux for a fresh graph snapshot. Emitted by `LiveMode::init`,
    /// the timer-driven refresh, and `Action::Refresh`.
    Discovery,
    /// A launch is starting a tmux session: the `--no-attach`
    /// subprocess, and the refresh when the launch ends without an
    /// attach.
    /// Emitted by `rebuild`, `mux new`, and `mux launch`.
    Launch,
    /// A content search is reading transcripts.
    Search,
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

/// Mux sessions the operator just returned from, plus the agent
/// sessions linked to them, whose rows still show the pre-hand-off
/// snapshot (ADR 0108). The renderer dims these rows and puts a
/// spinner in their attach-glyph cell until the follow-up refresh
/// lands.
#[derive(Debug, Clone)]
pub struct PendingHandoff {
    pub muxes: BTreeSet<MuxSessionId>,
    pub sessions: BTreeSet<AgentSessionId>,
    pub started_at: Instant,
    /// Set by the reducer, taken by the runtime, which then spawns
    /// the background refresh that settles the hand-off.
    refresh_requested: bool,
}

/// What the status bar says when an action on a row is refused because
/// the row still awaits the refresh after a hand-off (ADR 0132).
pub const HANDOFF_PENDING_HINT: &str = "refreshing after tmux · wait for the row to update";

impl App {
    /// Build a fresh app at the start of the run.
    pub fn new(config: RunConfig) -> Self {
        let initial_contains = config.initial_contains.clone();
        let kinds = config.search.kinds;
        let mut app = Self::build(config);
        if let Some(text) = initial_contains {
            // A search that cannot start (live preview off) says so
            // instead of leaving the lists quietly unfiltered.
            for effect in app.reduce_content(Msg::StartContentSearch { text, kinds }) {
                if let crate::tui::Effect::Toast(note) = effect {
                    app.post_toast(note);
                }
            }
        }
        app
    }

    fn build(config: RunConfig) -> Self {
        let mut config = config;
        if matches!(config.sessions_grouping, super::SessionsGrouping::None) {
            config.default_sort = super::Sort::Recency;
        }
        let sort = config.default_sort;
        let mux_recency = config.default_mux_recency;
        let preview_wrap = config.default_preview_wrap;
        let session_preview = config.default_session_preview;
        let filter = config.initial_filter.clone();
        let active_view = config.default_view;
        let grouping = match active_view {
            View::Sessions => super::Grouping::Sessions(config.sessions_grouping),
            View::Mux => super::Grouping::Mux(config.mux_grouping),
            View::Prs | View::Pinned | View::History | View::Search => {
                super::Grouping::default_for(active_view)
            }
        };
        let edge_meta_visible = config.show_edge_meta;
        Self {
            config,
            active_view,
            should_quit: false,
            handle: None,
            tree: RowTree::default(),
            tree_state: TreeState::new(),
            explorer: None,
            explorer_reflects: None,
            focus: Focus::Left,
            edge_meta_visible,
            preview: ScrollRegion::default(),
            loaded_at_epoch: None,
            status_message: None,
            messages: crate::tui::messages::message_log(),
            provider_status: ProviderStatus::default(),
            refresh_failure: None,
            in_flight_ops: Vec::new(),
            handoff: None,
            refresh_requested: false,
            pending_follow: None,
            nav_back: Vec::new(),
            pending_follow_row: None,
            content: None,
            content_stash: None,
            search_history: content_history::SearchHistory::default(),
            content_serial: 0,
            search_memory: None,
            scan_request: None,
            detail_request: None,
            search_auto_expanded: BTreeSet::new(),
            #[cfg(test)]
            tree_rebuilds: 0,
            content_generation: 0,
            pending_restore_focus: None,
            undo: crate::tui::undo::UndoStack::default(),
            preview_store: PreviewStore::new(),
            modal_stack: crate::tui::modal::ModalStack::new(),
            rename_node: None,
            toasts: Toasts::new(),
            sort,
            mux_recency,
            preview_wrap,
            session_preview,
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
            tree: self.tree_state.clone(),
        }
    }

    /// Restore a saved view state into the active fields.
    fn restore_active_state(&mut self, slot: ViewStateSlot) {
        self.filter = slot.filter;
        self.grouping = slot.grouping;
        self.tree_state = slot.tree;
    }

    /// Switch the active view to `target`, saving the previous
    /// view's state into `view_states` and loading the target's
    /// state (or fresh defaults on first visit). Per ADR 0031 sort
    /// stays global, so callers don't touch it here.
    ///
    /// Persistence is the caller's concern — the [`Msg::SwitchView`]
    /// reducer arm emits [`Effect::Persist`] after invoking this,
    /// per ADR 0085 contract 2 Phase E.
    ///
    /// Returns `true` on the first visit to `target`, whose state
    /// starts from defaults.
    pub(crate) fn switch_to_view(&mut self, target: View) -> bool {
        let from = self.active_view;
        if from == target {
            return false;
        }
        let saved = self.snapshot_active_state();
        self.view_states.insert(from, saved);
        let restored = self.view_states.remove(&target);
        let first_visit = restored.is_none();
        let loaded = restored.unwrap_or_else(|| ViewStateSlot::defaults_for(target));
        self.restore_active_state(loaded);
        self.active_view = target;
        self.force_recency_for_flat_sessions();
        self.status_message = None;
        first_visit
    }

    /// Open a view's tree the way a first load does: the launch
    /// context's group expanded and selected, or else the first group
    /// expanded.
    /// After a regroup the old scroll offset and selection mean nothing
    /// in the new layout: open the default groups, select the first
    /// row, and scroll to the top so every group header is on screen.
    fn show_top_of_tree(&mut self) {
        self.tree_state
            .set_expanded(tree::initial_expanded_rows(&self.tree));
        if let Some(first) = self.visible_rows().first().map(|row| row.id.clone()) {
            self.set_selection(first);
        }
        self.tree_state.scroll_to_top();
    }

    fn seed_first_visit(&mut self) {
        self.tree_state
            .set_expanded(tree::initial_expanded_rows(&self.tree));
        let launch = self.tree.rows.iter().find_map(|row| match &row.kind {
            RowKind::Group(group) if group.is_launch_context => Some(row.id.clone()),
            _ => None,
        });
        match launch {
            Some(id) => self.set_selection(id),
            None => {
                if let Some(first) = self.visible_rows().first().map(|row| row.id.clone()) {
                    self.set_selection(first);
                }
            }
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
        match crate::tui::actions::action_subject(self).node? {
            node @ NodeId::AgentSession(_) => {
                Some(("copied: agent_session id".to_string(), node.to_string()))
            }
            node @ NodeId::MuxSession(_) => Some(("copied: mux id".to_string(), node.to_string())),
            _ => None,
        }
    }

    /// Programmatically set the selection to a row id, recomputing
    /// the explorer. Used by the search overlay to land
    /// the operator on a picked match without typing j/k. No-op
    /// when the id isn't currently visible (the overlay would have
    /// rejected it during commit; the guard here is defensive).
    pub fn set_selection(&mut self, id: RowId) {
        // A caller-driven jump (search overlay commit, etc.) targets a
        // row id with no meaningful "current position", so between
        // copies of a duplicated row it lands on the first that shows.
        if !self.tree_state.select(&self.tree.rows, id) {
            return;
        }
        self.status_message = None;
        // A jump shows the row's node even when it lands on the row
        // already selected and the right pane has drilled away from it.
        self.explorer_reflects = None;
        self.recompute_explorer();
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

    /// Preview-pane wrap mode.
    pub fn preview_wrap(&self) -> super::WrapMode {
        self.preview_wrap
    }

    /// What the Preview shows for agent sessions in tmux.
    pub fn session_preview(&self) -> super::SessionPreview {
        self.session_preview
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
        if let Some(wrap) = persisted.preview_wrap {
            self.preview_wrap = wrap;
        }
        if let Some(choice) = persisted.session_preview {
            self.session_preview = choice;
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
                    tree: TreeState::new(),
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
            preview_wrap: Some(self.preview_wrap),
            session_preview: Some(self.session_preview),
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
        self.tree_state.selected()
    }

    /// The active view's tree state: its selection, folds, and the
    /// window the last frame drew.
    pub fn tree_state(&self) -> &TreeState<RowId> {
        &self.tree_state
    }

    /// The right pane's explorer on the current focused node. `None`
    /// until the reducer has resolved a selection.
    pub fn explorer(&self) -> Option<&GraphExplorer> {
        self.explorer.as_ref()
    }

    /// The app's data as the explorer reads it: the snapshot, the theme,
    /// whether link rows show their edge meta, and the left pane's
    /// selection. `None` before the first snapshot.
    pub fn graph_source(&self) -> Option<GraphSource<'_>> {
        Some(GraphSource {
            snapshot: self.handle.as_ref()?.snapshot(),
            home: home_for_config(&self.config),
            theme: self.theme(),
            edge_meta: self.edge_meta_visible,
            left: self.tree_state.selected(),
        })
    }

    /// The rows the explorer lists, top to bottom: what its cursor's
    /// index counts. Empty with no explorer.
    pub fn explorer_rows(&self) -> Vec<ExplorerRow> {
        let (Some(explorer), Some(source)) = (self.explorer.as_ref(), self.graph_source()) else {
            return Vec::new();
        };
        explorer
            .entries(&source)
            .into_iter()
            .map(|entry| entry.host)
            .collect()
    }

    /// The explorer cursor's row, which the Preview, the actions, and the
    /// status hint read.
    pub fn explorer_cursor_row(&self) -> Option<ExplorerRow> {
        let explorer = self.explorer.as_ref()?;
        Some(explorer.cursor_entry(&self.graph_source()?)?.host)
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
        self.tree_state.is_expanded(id)
    }

    /// Rows the right panel's Preview has been scrolled from its anchor:
    /// down from the top, or up from a transcript's newest turns.
    pub fn preview_scroll(&self) -> u16 {
        u16::try_from(self.preview.away()).unwrap_or(u16::MAX)
    }

    /// The Preview's scroll region, which its body places and draws.
    pub fn preview_region(&self) -> ScrollRegion {
        self.preview
    }

    /// Return the Preview to its anchor: the top, a transcript's newest
    /// turns, or the search match it opens on.
    fn reset_preview_scroll(&mut self) {
        self.preview.reset();
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
        self.status_message
            .as_ref()
            .map(|status| status.text.as_str())
    }

    /// The status-bar message with its severity, which colors it.
    pub fn status(&self) -> Option<&Feedback> {
        self.status_message.as_ref()
    }

    /// Operation outcomes logged this session (ADR 0105).
    pub fn messages(&self) -> &crate::tui::messages::MessageLog {
        &self.messages
    }

    /// Log an operation outcome and show its summary in the status
    /// bar (ADR 0105).
    pub fn report(&mut self, entry: crate::tui::messages::LogEntry) {
        self.update(Msg::Report(Box::new(entry)));
    }

    /// The operations the undo key can still reverse.
    pub fn undo_stack(&self) -> &crate::tui::undo::UndoStack {
        &self.undo
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

    /// The pending hand-off, if a refresh hasn't settled it yet.
    pub fn handoff(&self) -> Option<&PendingHandoff> {
        self.handoff.as_ref()
    }

    /// Whether `row` shows values from before the pending hand-off.
    /// Any row that stands for an affected mux or session counts,
    /// whichever renderer draws it: a pin, a History record, a ghost
    /// mux, and the entities nested under a pin included.
    pub fn row_awaits_handoff(&self, row: &RowKind) -> bool {
        let Some(handoff) = &self.handoff else {
            return false;
        };
        match row {
            RowKind::MuxSession(mux) => handoff.muxes.contains(&mux.mux),
            RowKind::AgentSession(session) => handoff.sessions.contains(&session.session),
            _ => false,
        }
    }

    /// Whether `row` is frozen until the refresh after an action lands,
    /// whichever renderer draws it.
    pub fn row_is_frozen(&self, row: &crate::tui::rows::Row) -> bool {
        self.row_awaits_handoff(&row.kind) || self.row_id_awaits_handoff(&row.id)
    }

    /// Whether the row with `id` awaits the hand-off, judged by the
    /// entity it stands for.
    pub fn row_id_awaits_handoff(&self, id: &RowId) -> bool {
        let Some(handoff) = &self.handoff else {
            return false;
        };
        let Some(node) = crate::tui::actions::subject_of_row(self, id).node else {
            return false;
        };
        match node {
            NodeId::MuxSession(mux) => handoff.muxes.contains(&mux),
            NodeId::AgentSession(session) => handoff.sessions.contains(&session),
            _ => false,
        }
    }

    /// Whether the selected left-pane row still shows values from
    /// before the pending hand-off (ADR 0132).
    pub fn selected_row_awaits_handoff(&self) -> bool {
        let (Some(_), Some(selection)) = (&self.handoff, self.tree_state.selected()) else {
            return false;
        };
        let Some(row) = self.tree.rows.iter().find(|row| &row.id == selection) else {
            return false;
        };
        self.row_is_frozen(row)
    }

    /// Agent sessions the loaded snapshot links to `mux`.
    fn sessions_linked_to(&self, mux: &MuxSessionId) -> Vec<AgentSessionId> {
        self.handle
            .as_ref()
            .map(|handle| sessions_linked_to_mux(handle.snapshot(), mux))
            .unwrap_or_default()
    }

    /// Take the hand-off's refresh request. Returns `true` once per
    /// [`Msg::HandoffReturned`] batch; the runtime answers it with a
    /// background refresh that rescans the live classes.
    pub fn take_handoff_refresh_request(&mut self) -> bool {
        self.handoff
            .as_mut()
            .is_some_and(|handoff| std::mem::take(&mut handoff.refresh_requested))
    }

    /// Whether a hand-off refresh is already waiting to be spawned.
    pub fn handoff_refresh_pending(&self) -> bool {
        self.handoff
            .as_ref()
            .is_some_and(|handoff| handoff.refresh_requested)
    }

    /// Take the request for a background refresh that a state-changing
    /// action left. Returns `true` once per request.
    pub fn take_refresh_request(&mut self) -> bool {
        std::mem::take(&mut self.refresh_requested)
    }

    /// Ask the runtime to refresh in the background: the action that
    /// just ran changed something the views show.
    pub fn request_refresh(&mut self) {
        self.refresh_requested = true;
    }

    /// Look up a cached mux preview. Returns `None` if the mux
    /// hasn't been captured yet.
    pub fn mux_preview(&self, mux: &MuxSessionId) -> Option<&PreviewEntry> {
        self.preview_store.get(&PreviewSource::Pane(mux.clone()))
    }

    /// Loaded previews, keyed by source.
    pub fn previews(&self) -> &PreviewStore {
        &self.preview_store
    }

    /// Compose the selection's transcript at `width` columns so the
    /// Preview zone, which draws from `&App`, finds its lines cached.
    /// Draw calls this once per frame, before the zone renders; the
    /// composition is reused until the width or the transcript
    /// changes.
    pub(crate) fn prepare_preview(&mut self, width: u16) {
        let Some(source @ PreviewSource::Transcript(_)) = crate::tui::preview::preview_source(self)
        else {
            return;
        };
        let theme = self.theme().clone();
        let search = crate::tui::preview::search::preview_search(self);
        if let Some(PreviewEntry {
            content: PreviewContent::Transcript(preview),
            ..
        }) = self.preview_store.get_mut(&source)
        {
            crate::tui::preview::search::apply(preview, search.as_ref());
            preview.transcript.lines(&theme, width);
        }
    }

    /// What the Preview's scroll counts from: the search match a
    /// transcript opens on, a transcript's newest turns, or the top.
    fn preview_anchor(&self) -> Anchor {
        if !matches!(
            crate::tui::preview::preview_source(self),
            Some(PreviewSource::Transcript(_))
        ) {
            Anchor::Top
        } else if crate::tui::preview::search::preview_search(self)
            .is_some_and(|search| search.turn.is_some())
        {
            Anchor::Mark
        } else {
            Anchor::Bottom
        }
    }

    /// What the Preview zone is pointed at: the left selection and the
    /// explorer's focused node and cursor, by its position and by the
    /// row there (a refresh can put another row in the cursor's place). A
    /// change resets the Preview's scroll; nothing else a refresh does
    /// moves it.
    fn preview_focus(&self) -> PreviewFocus {
        (
            self.tree_state.selected().cloned(),
            self.explorer.as_ref().map(|explorer| {
                (
                    explorer.id().clone(),
                    explorer.cursor(),
                    explorer.cursor_key().cloned(),
                )
            }),
        )
    }

    /// Iterate the row tree, skipping rows whose ancestors are
    /// collapsed. Iteration order matches display order.
    pub fn visible_rows(&self) -> Vec<&Row> {
        self.tree_state.visible(&self.tree.rows)
    }

    /// Apply a single [`Msg`] to the state. Pure: no I/O, no panics,
    /// no clock reads.
    pub fn update(&mut self, msg: Msg) -> Vec<super::Effect> {
        let focus = self.preview_focus();
        let effects = self.reduce(msg);
        if self.preview_focus() != focus {
            // A different row's preview starts at its anchor: the top,
            // or a transcript's newest turns.
            self.reset_preview_scroll();
        }
        effects
    }

    fn reduce(&mut self, msg: Msg) -> Vec<super::Effect> {
        use super::Effect;
        // The empty-stack Backspace arming persists across consecutive
        // Backspace presses so the two-press "leave the right pane"
        // confirmation can complete. An intervening operator action
        // (navigation, focus cycle, …) clears it so the next Backspace
        // re-surfaces the hint instead of jumping straight to the focus
        // shift. A drawn frame is not a message, so the frames between
        // the two presses leave the arm alone.
        if !matches!(msg, Msg::Explorer(tuikit::explorer::ExplorerKey::Back))
            && let Some(explorer) = self.explorer.as_mut()
        {
            explorer.disarm();
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
                self.set_data(snapshot, tree, loaded_at_epoch, initial_selection_hint);
                self.scan_new_sessions();
                self.apply_pending_follow();
                self.apply_restore_focus();
            }
            Msg::SetTree(tree) => {
                self.set_tree(tree);
            }
            Msg::SelectRow(id) => {
                self.select_row_expanding(*id);
            }
            Msg::OpenEntity(node) => {
                self.open_entity(&node);
            }
            Msg::NavDown => self.move_selection(|state, rows| state.step(rows, 1)),
            Msg::NavUp => self.move_selection(|state, rows| state.step(rows, -1)),
            Msg::PageDown => self.move_selection(TreeState::page_down),
            Msg::PageUp => self.move_selection(TreeState::page_up),
            Msg::Home => self.move_selection(TreeState::home),
            Msg::End => self.move_selection(TreeState::end),
            Msg::ToggleExpand => {
                self.tree_state.toggle(&self.tree.rows);
                self.queue_details();
            }
            Msg::ExpandRow => {
                self.tree_state.expand(&self.tree.rows);
                self.queue_details();
            }
            Msg::CollapseRow => {
                self.tree_state.collapse(&self.tree.rows);
            }
            Msg::Explorer(key) => effects.extend(self.explorer_key(key)),
            Msg::ToggleEdgeMeta => self.toggle_edge_meta(),
            Msg::NavBack if self.can_nav_back() => {
                if let Some(why) = self.nav_pop() {
                    effects.push(Effect::Toast(Feedback::warning(why)));
                }
                effects.push(Effect::Persist);
            }
            Msg::NavBack => self.left_back(),
            Msg::CycleFocus => {
                self.focus = match self.focus {
                    Focus::Left => Focus::Right,
                    Focus::Right => Focus::Left,
                };
            }
            Msg::ScrollPreviewBy(delta) => {
                // The region steps from the anchor the Preview shows now,
                // so a step toward a transcript's newest line, or above the
                // top, stops there at once.
                self.preview.set_anchor(self.preview_anchor());
                self.preview.scroll_by(delta);
            }
            Msg::SetStatus(message) => {
                self.status_message = message;
            }
            Msg::Copy { text, done } => effects.push(Effect::Copy { text, done }),
            Msg::Report(entry) => {
                let text = if entry.severity.needs_attention() {
                    format!(
                        "{} · {}",
                        entry.summary,
                        crate::tui::keybindings::messages_hint(self, "details")
                    )
                } else {
                    entry.summary.clone()
                };
                self.status_message = Some(Feedback::new(entry.severity, text));
                self.messages.push(*entry);
            }
            Msg::SetPreview { source, content } => {
                self.preview_store.insert(source, content);
            }
            Msg::PreviewLoadStarted(source) => {
                self.preview_store.start_loading(source);
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
            Msg::ForgeRefreshRequested => self.preview_store.mark_forge_stale(),
            Msg::HandoffReturned(mux) => {
                let sessions = self.sessions_linked_to(&mux);
                // The pane and the transcripts of the sessions in it
                // changed while the operator was there.
                self.preview_store
                    .mark_stale(&PreviewSource::Pane(mux.clone()));
                for session in &sessions {
                    self.preview_store
                        .mark_stale(&PreviewSource::Transcript(session.clone()));
                }
                let handoff = self.handoff.get_or_insert_with(|| PendingHandoff {
                    muxes: BTreeSet::new(),
                    sessions: BTreeSet::new(),
                    started_at: Instant::now(),
                    refresh_requested: false,
                });
                handoff.muxes.insert(mux);
                handoff.sessions.extend(sessions);
                handoff.refresh_requested = true;
            }
            Msg::ViewRegained => {
                let handoff = self.handoff.get_or_insert_with(|| PendingHandoff {
                    muxes: BTreeSet::new(),
                    sessions: BTreeSet::new(),
                    started_at: Instant::now(),
                    refresh_requested: false,
                });
                handoff.refresh_requested = true;
            }
            Msg::FollowEntity(node) => {
                self.pending_follow = Some(node);
                self.pending_follow_row = self.tree_state.selected().cloned();
            }
            Msg::HandoffSettled => {
                self.handoff = None;
                self.apply_pending_follow();
            }
            Msg::LaunchStarted(mux) => {
                let sessions = self.sessions_linked_to(&mux);
                let handoff = self.handoff.get_or_insert_with(|| PendingHandoff {
                    muxes: BTreeSet::new(),
                    sessions: BTreeSet::new(),
                    started_at: Instant::now(),
                    refresh_requested: false,
                });
                handoff.muxes.insert(mux);
                handoff.sessions.extend(sessions);
            }
            Msg::LaunchAborted(mux) => {
                let sessions = self.sessions_linked_to(&mux);
                if let Some(handoff) = &mut self.handoff {
                    handoff.muxes.remove(&mux);
                    for session in &sessions {
                        handoff.sessions.remove(session);
                    }
                    if handoff.muxes.is_empty()
                        && handoff.sessions.is_empty()
                        && !handoff.refresh_requested
                    {
                        self.handoff = None;
                    }
                }
            }
            Msg::AttachSelected => {
                use crate::tui::effect::ExecSpec;
                if self.selected_row_awaits_handoff() {
                    effects.push(Effect::Toast(Feedback::warning(HANDOFF_PENDING_HINT)));
                    return effects;
                }
                if let Some((session, open)) = crate::tui::actions::viewer_open_target(self) {
                    effects.push(Effect::Exec(ExecSpec::ViewSession(session, Some(open))));
                    return effects;
                }
                if let Some(effect) = crate::tui::actions::default_effect(self) {
                    effects.push(effect);
                    return effects;
                }
                match crate::tui::actions::resolve_attach_target(self) {
                    Ok(target) => effects.push(Effect::Exec(ExecSpec::AttachMux(target))),
                    Err(reason) => effects.push(Effect::Toast(Feedback::warning(
                        crate::tui::actions::attach_disabled_reason(&reason),
                    ))),
                }
            }
            Msg::ResumeSelected => {
                use crate::tui::effect::ExecSpec;
                if self.selected_row_awaits_handoff() {
                    effects.push(Effect::Toast(Feedback::warning(HANDOFF_PENDING_HINT)));
                    return effects;
                }
                let subject = crate::tui::actions::action_subject(self);
                if subject == crate::tui::actions::Subject::default() {
                    effects.push(Effect::Toast(Feedback::warning("resume: nothing selected")));
                    return effects;
                }
                let Some(crate::model::NodeId::AgentSession(session_id)) = subject.node else {
                    effects.push(Effect::Toast(Feedback::warning(
                        "resume: select an agent session",
                    )));
                    return effects;
                };
                let Some(handle) = self.snapshot_handle() else {
                    effects.push(Effect::Toast(Feedback::warning(
                        "resume: no snapshot loaded",
                    )));
                    return effects;
                };
                let target =
                    crate::tui::resume::resolve_resume_target_in(handle.snapshot(), &session_id);
                match &target {
                    crate::tui::resume::ResumeTarget::Launch { .. } => {
                        effects.push(Effect::Exec(ExecSpec::Resume(target)));
                    }
                    _ => effects.push(Effect::Toast(Feedback::warning(
                        crate::tui::resume::resume_disabled_reason(&target),
                    ))),
                }
            }
            Msg::ViewSelected => {
                use crate::tui::effect::ExecSpec;
                if let Some((session, open)) = crate::tui::actions::viewer_open_target(self) {
                    effects.push(Effect::Exec(ExecSpec::ViewSession(session, Some(open))));
                    return effects;
                }
                match crate::tui::actions::resolve_view_session(self) {
                    Ok(session_id) => {
                        effects.push(Effect::Exec(ExecSpec::ViewSession(session_id, None)));
                    }
                    Err(reason) => effects.push(Effect::Toast(Feedback::warning(
                        crate::tui::viewer::viewer_disabled_reason(&reason),
                    ))),
                }
            }
            Msg::SwitchView(view) => {
                let before = self.active_view;
                let first_visit = self.switch_to_view(view);
                self.rebuild_tree_in_place();
                if first_visit {
                    self.seed_first_visit();
                    // Seeding starts the view from its defaults; a Search
                    // view that opens on results opens its rows again.
                    self.search_auto_expanded.clear();
                    self.auto_expand_search_rows();
                }
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
                self.show_top_of_tree();
                effects.push(Effect::Persist);
            }
            Msg::StartContentSearch { .. }
            | Msg::PreviewContentSearch { .. }
            | Msg::ApplyContentSearch { .. }
            | Msg::DiscardContentPreview
            | Msg::KeepContentSearch { .. }
            | Msg::PageSearchHistory(_)
            | Msg::ShowSearchHistory(_)
            | Msg::OpenSearchHistory
            | Msg::OpenContentSearchModal
            | Msg::OpenSearchOptions
            | Msg::ToggleSearchGranularity
            | Msg::ApplySearchEntry
            | Msg::OpenPaneHit { .. }
            | Msg::OpenSelectedPane
            | Msg::HistoryDetails { .. }
            | Msg::ContentPanes { .. }
            | Msg::OpenContentHit { .. }
            | Msg::ClearContentSearch
            | Msg::RescanContent
            | Msg::SetContentGroup(_)
            | Msg::ContentDetails { .. }
            | Msg::ClearFilters
            | Msg::ContentHits { .. } => return self.reduce_content(msg),
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
            Msg::SetPreviewWrap(wrap) => {
                self.preview_wrap = wrap;
                self.reset_preview_scroll();
                effects.push(Effect::Persist);
            }
            Msg::SetSessionPreview(choice) => {
                self.session_preview = choice;
                self.reset_preview_scroll();
                effects.push(Effect::Persist);
            }
            Msg::CommitRename(value) => {
                let effect = self.rename_commit_effect(&value);
                effects.push(effect);
            }
            Msg::CommitSlotRename { slot, new_name } => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::RenameSlot { slot, new_name },
                ));
            }
            Msg::OpenControlsEditor(which) => {
                // The editor opens over the panel that asked, so Esc there
                // comes back to it and Ctrl-C closes both.
                let step = crate::tui::widgets::controls::editor(which, &self.controls_context());
                self.open_modal_step(step);
            }
            Msg::OpenWorktreeStep { action, context } => {
                // The step opens over the menu that asked, so Esc there
                // comes back to it and a commit or Ctrl-C closes both.
                if let Some(step) = crate::tui::widgets::worktree_menu::step(action, &context) {
                    self.open_modal_step(step);
                }
            }
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
            Msg::OpenRowMenu => match crate::tui::actions::row_menu_for(self) {
                Some(menu) => self.open_row_menu(menu),
                None => effects.push(Effect::Toast(Feedback::warning("no row actions here yet"))),
            },
            Msg::PinNode(node) => {
                effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::PinNode {
                    node,
                }));
            }
            Msg::Undo => match self.undo.pop() {
                None => effects.push(Effect::Toast(Feedback::warning("nothing to undo"))),
                Some(entry) => {
                    self.pending_restore_focus = entry.focus(self.active_view() == View::Pinned);
                    effects.push(Effect::WriteStore(entry.into_op()));
                }
            },
            Msg::UndoRecorded(entry) => self.undo.push(entry),
            Msg::TogglePin => {
                // `p` acts on the entity in view: it unpins what is
                // already pinned and pins the rest, each at once. An
                // entity several pins point at asks which to remove.
                let pins = self.snapshot_handle().map_or_else(Vec::new, |handle| {
                    crate::tui::actions::pins_for(
                        handle.snapshot(),
                        &crate::tui::actions::action_subject(self),
                    )
                });
                if pins.is_empty() {
                    match crate::tui::actions::pin_subject(self) {
                        Err(why) => {
                            effects.push(Effect::Toast(Feedback::warning(format!("pin: {why}"))));
                        }
                        Ok(crate::tui::actions::PinSubject::Node(node)) => {
                            effects.push(Effect::WriteStore(
                                crate::tui::effect::StoreOp::PinNode { node },
                            ));
                        }
                        Ok(crate::tui::actions::PinSubject::Record(record_id)) => {
                            effects.push(Effect::WriteStore(
                                crate::tui::effect::StoreOp::PinRecord { record_id },
                            ));
                        }
                    }
                } else if let [only] = pins.as_slice() {
                    effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::Unpin {
                        slot: only.clone(),
                    }));
                } else {
                    self.open_pin_menu(pins);
                }
            }
            Msg::Unpin(slot) => {
                effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::Unpin {
                    slot,
                }));
            }
            Msg::OpenSlotEdit(slot) => match self.pin_edit_init(&slot) {
                Some(init) => {
                    self.open_pin_edit_form(crate::tui::widgets::pin_edit::PinEditForm::new(init));
                }
                None => effects.push(Effect::Toast(Feedback::warning(format!(
                    "no pin named `{slot}`"
                )))),
            },
            Msg::CommitPinEdit(request) => {
                effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::SavePin {
                    request,
                }));
            }
            Msg::OpenSlotRename(slot) => self.open_pin_rename(slot),
            Msg::OpenSlotRetarget(pin) => {
                let candidates = self
                    .snapshot_handle()
                    .map(|handle| crate::tui::actions::retarget_candidates(handle.snapshot(), &pin))
                    .unwrap_or_default();
                if candidates.is_empty() {
                    effects.push(Effect::Toast(Feedback::warning(format!(
                        "pin `{pin}`: there is nothing else of its kind to point it at"
                    ))));
                } else {
                    self.open_picker(crate::tui::widgets::picker::retarget(pin, candidates));
                }
            }
            Msg::CommitRetarget { pin, target } => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::RetargetSlot { slot: pin, target },
                ));
            }
            Msg::RefreshSlot(slot) => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::RefreshSlot { slot },
                ));
            }
            Msg::RevealSlot(slot) => {
                if let Err(why) = self.reveal_slot(&slot) {
                    effects.push(Effect::Toast(Feedback::warning(why)));
                } else {
                    effects.push(Effect::Persist);
                }
            }
            Msg::AttachPane => {
                let Some(viewer) = self.pane_viewer() else {
                    return effects;
                };
                let mux = crate::model::MuxSessionId::new(viewer.content().pane.mux.native_id());
                match crate::tui::actions::attach_target_for_mux(self, &mux) {
                    Ok(target) => {
                        self.close_pane_viewer();
                        effects.push(Effect::Exec(crate::tui::effect::ExecSpec::AttachMux(
                            target,
                        )));
                    }
                    Err(reason) => {
                        if let Some(viewer) = self.pane_viewer_mut() {
                            viewer.set_note(Feedback::warning(
                                crate::tui::actions::attach_disabled_reason(&reason),
                            ));
                        }
                    }
                }
            }
            Msg::RevealSelected => {
                let subject = crate::tui::actions::action_subject(self);
                let result = match (&subject.pin, &subject.node) {
                    (Some(pin), _) => self.reveal_slot(pin),
                    (None, Some(node)) => self
                        .reveal_node(node, subject.record.clone())
                        .map_err(|why| format!("that row {why}")),
                    (None, None) => Err("nothing to reveal on this row".to_string()),
                };
                match result {
                    Ok(()) => effects.push(Effect::Persist),
                    Err(why) => effects.push(Effect::Toast(Feedback::warning(why))),
                }
            }
            Msg::PinRecord(record_id) => {
                effects.push(Effect::WriteStore(crate::tui::effect::StoreOp::PinRecord {
                    record_id,
                }));
            }
            Msg::ForgetRecord(record_id) => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::ForgetRecord { record_id },
                ));
            }
            Msg::ReapRecords(ids) => {
                effects.push(Effect::WriteStore(
                    crate::tui::effect::StoreOp::ReapRecords { ids },
                ));
            }
            Msg::ShowRebuildPlan(reference) => {
                effects.push(Effect::Exec(
                    crate::tui::effect::ExecSpec::ShowRebuildPlan { reference },
                ));
            }
            Msg::RebuildReference(reference) => {
                effects.push(Effect::Exec(crate::tui::effect::ExecSpec::Rebuild {
                    reference,
                    attach: false,
                }));
            }
            Msg::OpenCleanUp => {
                if let Some(handle) = self.snapshot_handle() {
                    let now = crate::tui::rows::tree_current_unix_epoch();
                    let candidates = crate::history::reap::candidates(
                        handle.snapshot(),
                        &self.config().history,
                        now,
                        None,
                        None,
                    );
                    if candidates.is_empty() {
                        // Nothing to choose from: a notice says why.
                        let notice = crate::tui::widgets::clean_up::empty_notice();
                        self.open_modal(crate::tui::Modal::Dialog(Box::new(notice)));
                    } else {
                        self.open_clean_up(crate::tui::widgets::clean_up::checklist(
                            &candidates,
                            now,
                        ));
                    }
                }
            }
            Msg::OpenNewMuxForm => {
                let form = crate::tui::runtime::new_mux_form(self);
                self.open_new_mux_form(form);
            }
            Msg::OpenMuxLaunchForm => {
                let seeded_cwd = crate::tui::runtime::derive_mux_form_cwd(self)
                    .unwrap_or_else(|| std::env::var("HOME").unwrap_or_else(|_| "/".to_string()));
                let seeded_mux_name =
                    crate::tui::runtime::derive_mux_launch_name(self).unwrap_or_default();
                let known_harness_keys = crate::tui::runtime::known_harness_keys();
                let known_mux_names = crate::tui::runtime::known_live_mux_names(self);
                let default_harness = known_harness_keys.first().cloned().unwrap_or_default();
                let repo = seeded_cwd.clone();
                let candidates = crate::tui::runtime::known_cwd_candidates(self, Some(&seeded_cwd));
                let form = crate::tui::widgets::mux_launch::MuxLaunchForm::new(
                    default_harness,
                    seeded_cwd,
                    seeded_mux_name,
                    known_harness_keys,
                    known_mux_names,
                )
                .with_defaults(crate::launch_default::LaunchDefaults::new(
                    self.config().launch.clone(),
                ))
                .with_cwd_candidates(candidates)
                .with_worktree_repo(repo);
                self.open_mux_launch_form(form);
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
    /// current selection: an alias or mux rename, or a toast
    /// explaining why nothing can be renamed.
    fn rename_commit_effect(&mut self, value: &str) -> super::Effect {
        use super::Effect;
        let renamed = self.rename_node.take();
        let target = match renamed {
            Some(NodeId::AgentSession(id)) => Some(RowId::AgentSession(NodeId::AgentSession(id))),
            Some(NodeId::MuxSession(id)) => Some(RowId::MuxSession(NodeId::MuxSession(id))),
            Some(_) => None,
            None => self.tree_state.selected().cloned(),
        };
        match target {
            Some(RowId::AgentSession(NodeId::AgentSession(id))) => {
                let trimmed = value.trim().to_string();
                let new_display_name = (!trimmed.is_empty()).then_some(trimmed);
                Effect::WriteStore(crate::tui::effect::StoreOp::CommitAliasRename {
                    session_id: id,
                    new_display_name,
                })
            }
            Some(RowId::Pin { pin_id }) => Effect::Toast(Feedback::warning(format!(
                "rename the pin with `{} pin rename {pin_id} <new name>`",
                crate::invocation::invoked_name()
            ))),
            Some(RowId::MuxSession(NodeId::MuxSession(mux_id))) => {
                let new_name = value.trim().to_string();
                if new_name.is_empty() {
                    return Effect::Toast(Feedback::warning("mux rename: name cannot be empty"));
                }
                Effect::WriteStore(crate::tui::effect::StoreOp::CommitMuxRename {
                    mux_id,
                    new_name,
                })
            }
            _ => Effect::Toast(Feedback::error("rename: lost selection before commit")),
        }
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

/// Agent sessions with an active `LinkedToMux` candidate pointing at
/// `mux`. Any candidate counts, not just the preferred one: the
/// hand-off may have changed which session the resolver prefers.
pub(in crate::tui) fn sessions_linked_to_mux(
    snapshot: &GraphSnapshot,
    mux: &MuxSessionId,
) -> Vec<AgentSessionId> {
    snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == crate::model::RelationKind::LinkedToMux
                && matches!(link.state, crate::model::LinkState::Active)
                && matches!(link.target_node_id(), Some(NodeId::MuxSession(id)) if id == mux)
        })
        .filter_map(|link| match &link.source {
            NodeId::AgentSession(id) => Some(id.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
