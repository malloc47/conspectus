//! The reducer's message type.

use super::*;

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
    /// context row instead of the leading row in the tree;
    /// later refreshes ignore the hint and prefer the retained
    /// selection.
    SetData {
        snapshot: SnapshotHandle,
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
    /// Set the left-panel selection to a specific row id.
    /// Used by the search overlay's Confirm outcome so
    /// its selection change flows through the reducer instead of a
    /// direct `App::set_selection` call inside a specialized runtime
    /// handler.
    SelectRow(Box<RowId>),
    /// Open a quick-select result: select its row, follow its
    /// relationships in the right pane, or switch to the view that lists
    /// it, whichever its outcome word promised.
    OpenEntity(NodeId),
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
    /// Right panel (graph explorer): move the cursor down one row
    /// in the flat row list.
    ExplorerNavDown,
    /// Right panel (graph explorer): move the cursor up one row.
    ExplorerNavUp,
    /// Right panel (graph explorer): snap the cursor to the first
    /// row in the flat list. Bound to `g` / `Home` on right-pane
    /// focus, matching the left tree.
    ExplorerHome,
    /// Right panel (graph explorer): snap the cursor to the last
    /// row in the flat list. Bound to `G` / `End` on right-pane
    /// focus.
    ExplorerEnd,
    /// Right panel (graph explorer): activate the highlighted row.
    /// On a group header this toggles the group's expansion; on a
    /// link row it drills into the neighbor and pushes a breadcrumb
    /// hop. No-op on unresolved-evidence rows in v1.
    ExplorerActivate,
    /// Right panel (graph explorer): expand or collapse the `Other`
    /// zone when the cursor is on its header; a status hint
    /// otherwise. Bound to `e`, which the left pane ignores.
    ExplorerToggleGroup,
    /// Right panel (graph explorer): toggle the visibility of the
    /// link rows' trailing `provenance · confidence · state` meta
    /// line. The default is hidden; the `★` resolver-winner
    /// marker and `▲` group-level conflict aggregate stay visible
    /// regardless.
    ToggleEdgeMeta,
    /// Right panel (graph explorer): toggle the Expanded Node Detail
    /// view. Swaps the Node zone's top-5 render for the
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
    /// scrolls down (toward the end of the content), negative
    /// scrolls up. The reducer clamps the offset at zero; a
    /// transcript counts its offset up from its newest line.
    ScrollPreviewBy(i32),
    /// Set or clear the transient status-bar message, drawn in its
    /// severity's color. `None` clears any prior message.
    SetStatus(Option<tuikit::feedback::Feedback>),
    /// Show a short notice in a toast. An overlay that refuses input
    /// and stays open sends this to say why: the toast draws over every
    /// overlay, where the status line can be covered.
    Toast(tuikit::feedback::Feedback),
    /// Copy `text` to the clipboard and, when it worked, say `done` in a
    /// toast. Overlays that copy (the value modal, Messages) send this:
    /// the clipboard write is the executor's.
    Copy {
        text: String,
        done: String,
    },
    /// Log an operation outcome (ADR 0105) and show its summary as
    /// the status message, with a pointer to the Messages overlay
    /// for warnings and errors.
    Report(Box<crate::tui::messages::LogEntry>),
    /// Store a loaded preview in the cache (ADR 0112 §5). The
    /// runtime dispatches this when a pane capture or a transcript
    /// read for the selection finishes.
    SetPreview {
        source: PreviewSource,
        content: PreviewContent,
    },
    /// The runtime started loading a preview, so the next selection
    /// pass doesn't start another.
    PreviewLoadStarted(PreviewSource),
    /// Update the provider availability status surfaced as chips
    /// in the status bar. The runtime populates this from
    /// discovery diagnostics and env-var toggles.
    SetProviderStatus(ProviderStatus),
    /// Record that the most recent refresh failed. The previous
    /// good snapshot remains in place; this message surfaces a
    /// stale indicator in the header or status bar.
    SetRefreshFailure(String),
    /// Start tracking a new in-flight async operation.
    /// The reducer stamps `Instant::now()` and stores the op; the
    /// status bar renders one animated spinner chip per active op.
    /// Idempotent by `InFlightKind`: starting the same kind twice
    /// replaces the prior record's `started_at` and label.
    InFlightStart {
        kind: InFlightKind,
        label: String,
    },
    /// The operator asked for a hard refresh: cached pull-request
    /// previews reload the next time they show (ADR 0116).
    ForgeRefreshRequested,
    /// Mark an in-flight async operation complete.
    /// Removes the matching kind from the tracker; a no-op if no
    /// op with that kind is currently in flight.
    InFlightFinish(InFlightKind),
    /// The operator returned from a tmux attach to `mux` (ADR 0108).
    /// Marks the mux and its linked sessions as awaiting a refresh,
    /// marks their cached previews for reload, and asks the runtime
    /// for a background refresh. Repeats before that refresh lands add to
    /// the same pending set.
    HandoffReturned(MuxSessionId),
    /// The TUI's tmux pane came back into view after the operator was
    /// in another session. Asks for the same background refresh as a
    /// returned attach, without marking any row as awaiting it.
    ViewRegained,
    /// After the next refresh that settles the pending action, move to
    /// the view where `node` now lives and select its row.
    FollowEntity(crate::model::NodeId),
    /// A refresh spawned after the last hand-off landed (or failed);
    /// the pending rows show current values again.
    HandoffSettled,
    /// A launch is starting `mux` (ADR 0127). Marks the mux row
    /// and its linked sessions with the hand-off's dim-and-spinner
    /// treatment so the row being acted on shows progress. Unlike
    /// [`Msg::HandoffReturned`] it asks for no refresh: the attach
    /// that follows sends that.
    LaunchStarted(MuxSessionId),
    /// The launch of `mux` ended without an attach. Drops its mark; an
    /// attach that did run sends [`Msg::HandoffReturned`] instead.
    LaunchAborted(MuxSessionId),
    /// Nested-reducer entry point for the transcript viewer
    /// (ADR 0085 contract 3). The reducer arm
    /// pops the top viewer state, runs it through
    /// `crate::viewer::input::reduce`, and pushes the new state
    /// back on `ViewerEffect::None` or leaves the stack popped
    /// on `ViewerEffect::Close`. No-op when the top of the modal
    /// stack isn't the viewer.
    Viewer(crate::viewer::input::ViewerMsg),
    /// A key of the viewer's find prompt.
    ViewerFind(crate::viewer::find::ViewerFindMsg),
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
    /// Confirm the session or mux rename prompt's typed value (ADR
    /// 0029). Reducer resolves the entity the prompt opened for (or the
    /// selected row) and emits either a mux rename, an agent-session
    /// alias rename, or a "lost selection" toast. The alias-rename
    /// branch also chains an optional native mux rename inside the
    /// executor per lockstep. A pin's rename prompt commits
    /// [`Msg::CommitSlotRename`] instead.
    CommitRename(String),
    /// Commit the worktree menu's "New worktree" branch input.
    /// Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreeCreate)`.
    CommitWorktreeCreate {
        repo_root: String,
        branch: String,
    },
    /// Commit the worktree menu's "Remove worktree" confirm.
    /// Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreeRemove)`.
    CommitWorktreeRemove {
        repo_root: String,
        branch: String,
        force: bool,
    },
    /// Commit the worktree menu's "Merge back & close" confirm.
    /// Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreeMerge)`.
    CommitWorktreeMerge {
        worktree_root: String,
        target: Option<String>,
    },
    /// Commit the worktree menu's "Close down stream" choice.
    /// Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreeCloseDown)`. `discard`
    /// drops the branch; otherwise it is merged back first.
    CommitWorktreeCloseDown {
        repo_root: String,
        branch: String,
        discard: bool,
    },
    /// Commit the worktree menu's "Prune merged worktrees" confirm.
    /// Reducer emits
    /// `Effect::WriteStore(StoreOp::WorktreePrune)`.
    CommitWorktreePrune {
        repo_root: String,
    },
    /// Commit the bare tmux `new-session` form (ADR 0095). Reducer emits `Effect::Exec(ExecSpec::MuxNew)` so
    /// the runtime re-execs into `conspectus mux new` with the same
    /// UX (alt-screen suspend → subprocess → refresh → attach) as
    /// `mux launch`.
    CommitMuxNew {
        name: String,
        cwd: String,
    },
    /// Mux action menu → open the bare-mux form. Reducer arm pushes
    /// the modal after seeding defaults from the current selection
    /// (ADR 0096).
    OpenNewMuxForm,
    /// Mux action menu → open the mux-launch form. Reducer arm pushes
    /// the modal after seeding defaults from the current selection
    /// (ADR 0096).
    OpenMuxLaunchForm,
    /// `.` — open the row action menu for the selected row.
    OpenRowMenu,
    /// Row menu → pin a history record as a slot.
    PinRecord(String),
    /// Row menu → pin a graph node as a slot.
    PinNode(crate::model::NodeId),
    /// `p` — pin the selected row, or unpin it at once when a pin
    /// already points at it.
    TogglePin,
    /// Row menu → remove a slot.
    Unpin(String),
    /// The undo key: reverse the newest recorded operation.
    Undo,
    /// The executor finished an operation that can be undone.
    UndoRecorded(crate::tui::undo::Undo),
    /// Pin menu → open the pin edit form.
    OpenSlotEdit(String),
    /// The pin edit form saved: rename the pin and set or drop overrides.
    CommitPinEdit(crate::tui::widgets::pin_edit::PinEditRequest),
    /// Slot menu → open the rename prompt.
    OpenSlotRename(String),
    /// The pin rename prompt confirmed a name: rename the slot (and a
    /// live mux with it). The prompt refuses a blank name itself.
    CommitSlotRename {
        slot: String,
        new_name: String,
    },
    /// Slot menu → open the retarget prompt.
    OpenSlotRetarget(String),
    /// Retarget picker → point the pin at the chosen entity (a node id
    /// or a history record id).
    CommitRetarget {
        pin: String,
        target: String,
    },
    /// Slot menu → copy the entity's current recipe into the slot.
    RefreshSlot(String),
    /// Slot menu → switch to the entity's native view and select it.
    RevealSlot(String),
    /// `Shift+Enter` / `Alt+Enter` → switch to the selected row's entity
    /// in its native view and select it; Backspace returns.
    RevealSelected,
    /// Pane viewer → attach to the mux the pane belongs to. A stopped or
    /// gone mux is refused with a note in the viewer.
    AttachPane,
    /// Row menu → run `conspectus rebuild <reference> --dry-run` and
    /// show the plan.
    ShowRebuildPlan(String),
    /// Row menu → run `conspectus rebuild <reference>`.
    RebuildReference(String),
    /// Row menu → remove one history record.
    ForgetRecord(String),
    /// Row menu → open the Clean up modal.
    OpenCleanUp,
    /// Clean up modal → remove exactly these history records.
    ReapRecords(Vec<String>),
    /// Commit the mux-launch form (ADR 0096).
    /// Reducer emits `Effect::Exec(ExecSpec::MuxLaunch)` so the
    /// runtime re-execs into `conspectus mux launch <harness> …`,
    /// refreshes discovery, then attaches. A pin slot is written only when the form asked for
    /// one.
    CommitMuxLaunch(crate::tui::widgets::mux_launch::MuxLaunchRequest),
    /// Switch the active row-tree view (ADR 0031). Reducer saves
    /// the current view's per-view slot, loads the target's slot
    /// (or fresh defaults on first visit), and re-derives the row
    /// tree from the held snapshot — projection-only, never
    /// triggers discovery (ADR 0085 contract 4).
    SwitchView(View),
    /// Update the active view's grouping (ADR 0031). Reducer
    /// mutates the projection state and re-derives the tree.
    SetGrouping(crate::tui::Grouping),
    /// Update the active row filter (ADR 0031). Reducer
    /// mutates the projection state and re-derives the tree.
    SetFilter(crate::filter::RowFilter),
    /// Search every session's transcript for this text and narrow the
    /// session lists to the sessions that match. The result stays in
    /// memory only.
    StartContentSearch {
        text: String,
        kinds: crate::viewer::search::ContentKinds,
    },
    /// Search while the `/` overlay is open: the scan runs and its
    /// results feed the overlay, but the session lists are not narrowed
    /// until [`Msg::ApplyContentSearch`].
    PreviewContentSearch {
        text: String,
        kinds: crate::viewer::search::ContentKinds,
        group: crate::viewer::search::Grouping,
        reach: crate::tui::widgets::search::Reach,
    },
    /// Keep this search as the filter on the session lists, and with
    /// `show_results` open the Search view on it. The previewed scan is
    /// reused when it is for the same query.
    ApplyContentSearch {
        text: String,
        kinds: crate::viewer::search::ContentKinds,
        group: crate::viewer::search::Grouping,
        reach: crate::tui::widgets::search::Reach,
        show_results: bool,
    },
    /// Keep the previewed search as the newest one in the Search view and
    /// open the view on it, without narrowing the lists. A scan still
    /// running keeps streaming into the view.
    KeepContentSearch {
        text: String,
        kinds: crate::viewer::search::ContentKinds,
        group: crate::viewer::search::Grouping,
        reach: crate::tui::widgets::search::Reach,
    },
    /// Show an older (negative) or newer (positive) remembered search in
    /// the Search view. It is not scanned again.
    PageSearchHistory(isize),
    /// Show the remembered search at this position, oldest first.
    ShowSearchHistory(usize),
    /// List the remembered searches in a picker.
    OpenSearchHistory,
    /// Open the contents search modal (what `s` does).
    OpenContentSearchModal,
    /// Open the contents search modal with its options pane showing.
    OpenSearchOptions,
    /// Flip the Search view between listing sessions and messages.
    ToggleSearchGranularity,
    /// Keep the search the Search view shows as the filter on the lists.
    ApplySearchEntry,
    /// Drop the previewed search and bring back the filter it replaced,
    /// if there was one.
    DiscardContentPreview,
    /// Open a mux pane's scrollback at a line a content search found.
    OpenPaneHit {
        pane: crate::content_search::PaneRef,
        line: usize,
        text: String,
        /// Select the pane's mux in the Mux view first, as a hit chosen in
        /// the modal does; a row of the Search view stays where it is.
        reveal: bool,
    },
    /// Open the scrollback of the pane the selected Search-view row names,
    /// on the line it found.
    OpenSelectedPane,
    /// Open the viewer on a session at the turn a content search found.
    OpenContentHit {
        session: crate::model::AgentSessionId,
        text: String,
        turn_index: usize,
    },
    /// Stop the content search and show every session again.
    ClearContentSearch,
    /// Read every transcript again for the active content search, as an
    /// explicit refresh asks.
    RescanContent,
    /// Show the content search results per session or per message.
    SetContentGroup(crate::viewer::search::Grouping),
    /// The matching turns of some sessions, read for the per-message
    /// grouping; stale ones are dropped like [`Msg::ContentHits`].
    ContentDetails {
        generation: u64,
        batch: Vec<(
            crate::content_search::SessionKey,
            Vec<crate::viewer::search::Hit>,
        )>,
    },
    /// The matching turns of some sessions for a remembered search that is
    /// not the running one; stale ones are dropped.
    HistoryDetails {
        serial: u64,
        batch: Vec<(
            crate::content_search::SessionKey,
            Vec<crate::viewer::search::Hit>,
        )>,
    },
    /// Clear every filter, the content search included.
    ClearFilters,
    /// Mux panes the running scan found, stamped like
    /// [`Msg::ContentHits`] and sent just before them.
    ContentPanes {
        generation: u64,
        batch: Vec<crate::content_search::PaneHits>,
    },
    /// Results of the running scan, stamped with the scan they belong
    /// to. Stale ones are dropped.
    ContentHits {
        generation: u64,
        batch: Vec<(
            crate::content_search::SessionKey,
            crate::viewer::search::SessionHits,
        )>,
        progress: crate::content_search::Progress,
        done: bool,
    },
    /// Update the global sort (ADR 0031). Reducer mutates the
    /// projection state and re-derives the tree; the flat-
    /// sessions grouping forces recency regardless of the
    /// requested value.
    SetSort(crate::tui::Sort),
    /// Update the mux-view recency basis. Reducer
    /// stores the basis, forces `Sort::Recency` so the choice takes
    /// effect, and re-derives the tree.
    SetMuxRecency(crate::tui::MuxRecency),
    /// Change how the preview pane fits a captured mux pane
    /// (ADR 0106). Render-only: the tree is untouched.
    SetPreviewWrap(crate::tui::PreviewWrap),
    /// Choose what the Preview shows for agent sessions in tmux
    /// (ADR 0112). Render-only: the tree is untouched.
    SetSessionPreview(crate::tui::SessionPreview),
}
