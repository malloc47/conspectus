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
    /// Left panel: move selection down/up one visible row.
    NavDown,
    NavUp,
    /// Left panel: page through visible rows. The runtime passes
    /// the rendered viewport height so the reducer can decide how
    /// many rows constitute one page. Pass 1 if unknown.
    PageDown(u16),
    PageUp(u16),
    /// Left panel: post-layout viewport dimensions.
    /// The draw path dispatches this before
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
    /// viewport dimensions. The draw path
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
    /// Right panel (graph explorer): toggle expansion of the
    /// highlighted multi-link group. No-op when the cursor isn't
    /// on a header.
    ExplorerToggleGroup,
    /// Right panel (graph explorer): toggle the visibility of the
    /// link rows' trailing `provenance · confidence · state` meta
    /// line. The default is hidden; the `★` resolver-winner
    /// marker and `⚠` group-level conflict aggregate stay visible
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
    /// Mark an in-flight async operation complete.
    /// Removes the matching kind from the tracker; a no-op if no
    /// op with that kind is currently in flight.
    InFlightFinish(InFlightKind),
    /// Nested-reducer entry point for the transcript viewer
    /// (ADR 0085 contract 3). The reducer arm
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
    /// Carries the already-resolved `PinRemoveRequest`; the
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
    /// pin launch.
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
    /// Commit the mux-launch form (ADR 0096).
    /// Reducer emits `Effect::Exec(ExecSpec::MuxLaunch)` so the
    /// runtime re-execs into `conspectus mux launch <harness> …`,
    /// refreshes discovery, then attaches. No pin write; no sidecar.
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
    /// Update the global sort (ADR 0031). Reducer mutates the
    /// projection state and re-derives the tree; the flat-
    /// sessions grouping forces recency regardless of the
    /// requested value.
    SetSort(crate::tui::Sort),
    /// Update the mux-view recency basis. Reducer
    /// stores the basis, forces `Sort::Recency` so the choice takes
    /// effect, and re-derives the tree.
    SetMuxRecency(crate::tui::MuxRecency),
}
