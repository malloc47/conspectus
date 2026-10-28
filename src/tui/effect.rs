//! Effects (ADR 0085 contract 2): data values the reducer emits
//! to schedule side effects. The runtime owns the effect executor,
//! which is the only code that touches `&mut Terminal`, the mux
//! runner, `std::process`, or the filesystem. `App::update` returns
//! `Vec<Effect>`; the reducer never blocks on I/O.
//!
//! Variants exist only once the executor can run them, so the
//! reducer never emits an effect nobody can execute.

use crate::model::{AgentSessionId, MuxSessionId};
use crate::tui::actions::AttachTarget;
use crate::tui::resume::ResumeTarget;
use crate::tui::widgets::mux_launch::MuxLaunchRequest;

/// A side effect the reducer wants the runtime to schedule.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Ask the runtime to run a fresh discovery pass.
    ///
    /// `force_local` bypasses the daemon and runs a full cold scan
    /// — used after store-mutation effects so the resulting graph
    /// reflects the just-written change without waiting for the
    /// daemon's next tick.
    SpawnRefresh { force_local: bool },
    /// Post a transient status-bar toast. Non-blocking; input
    /// continues to flow to the underlying view.
    Toast(String),
    /// Persist the current TUI state to disk. Best-effort;
    /// failures are silently swallowed by the executor.
    Persist,
    /// Quit the event loop after the current frame.
    Quit,
    /// Run an external process. Terminal-suspending execs (attach
    /// into a mux client) and background subprocess launches
    /// (resume) both flow through here; the executor decides how to
    /// hand off the terminal per variant. Only the live-loop
    /// executor handles `Exec`; pure contexts (tests, snapshot mode)
    /// treat it as a no-op.
    Exec(ExecSpec),
    /// Run a mux backend op (ADR 0085 contract 2 Phase C). The
    /// executor owns the `MuxBackend` reference for the duration of
    /// the call; the reducer never talks to tmux directly. Pure
    /// contexts silently drop `RunMux` — snapshot mode has no live
    /// backend and tests use FakeTmux directly against
    /// `execute_effects_live` when they want to assert against
    /// mux ops.
    RunMux(MuxOp),
    /// Persist a user-authored declaration to disk (ADR 0085
    /// contract 2 Phase D). The reducer emits the store op; the
    /// executor performs the TOML write, schedules a follow-up
    /// force-local refresh so the row tree reflects the change,
    /// and posts a status message summarizing the outcome. The
    /// executor is the sole code in the TUI that writes to
    /// `.conspectus.toml` for reducer-emitted effects.
    WriteStore(StoreOp),
}

/// Description of a subprocess the executor should run. All variants
/// carry the resolved target data so the reducer never touches
/// `std::process`, the filesystem, or the terminal.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecSpec {
    /// Suspend the alt screen and attach to a mux session.
    /// Executor runs `tmux attach-session` (or `switch-client` when
    /// nested), waits for it to exit, re-enters the alt screen, and
    /// schedules a follow-up refresh so the row tree reflects any
    /// sessions that came and went during the attach.
    AttachMux(AttachTarget),
    /// Spawn the harness resume command in the background. Does not
    /// suspend the alt screen — resume is fire-and-forget so the
    /// operator can watch a new terminal window come up while
    /// staying in the TUI.
    Resume(ResumeTarget),
    /// Open the transcript viewer for an agent session. The executor
    /// picks native (parse transcript on disk into `ViewerState` +
    /// `App::open_viewer_modal`) vs external fallback (spawn
    /// `claude-history` etc.); both paths read the filesystem so the
    /// reducer never touches them. With a `ViewerOpen` the viewer
    /// lands on that search result with the query highlighted.
    ViewSession(AgentSessionId, Option<crate::viewer::state::ViewerOpen>),
    /// Read a mux pane's scrollback now and open it in a pager on the
    /// line a content search found, with `text` picked out. Reads the
    /// pane (a capture) and nothing else.
    PaneScrollback {
        pane: crate::content_search::PaneRef,
        line: usize,
        text: String,
    },
    /// Suspend the alt screen, re-exec into
    /// `conspectus mux new <name> --cwd <cwd> --no-attach`, refresh
    /// so the row tree picks up the new session, then attach.
    /// The operator supplied the fields via the form (ADR 0095).
    MuxNew { name: String, cwd: String },
    /// Suspend the alt screen, re-exec into
    /// `conspectus mux launch <harness> --name <name> …
    /// --no-attach`, refresh so the row tree picks up the new mux
    /// and its attributed harness session, then attach
    /// (ADR 0096). A pin slot is written only when the form asked
    /// for one (`--pin`).
    MuxLaunch { request: MuxLaunchRequest },
    /// Run `conspectus rebuild <reference> --no-attach` behind a
    /// spinner, then refresh, so a gone pin or record comes back
    /// without leaving the TUI. With `attach`, the rebuilt mux is
    /// attached to as soon as it is up, as a launch always did.
    Rebuild { reference: String, attach: bool },
    /// Run `conspectus rebuild <reference> --dry-run` behind a spinner
    /// and show the plan it prints.
    ShowRebuildPlan { reference: String },
}

/// A mux backend op the executor should run. The reducer emits this
/// via `Effect::RunMux(...)`; the executor holds the sole
/// `MuxBackend` reference and performs the call. Other mux
/// mutations (rename, new session, send-keys) run inside the
/// executors for the store ops and launches that need them.
#[derive(Debug, Clone, PartialEq)]
pub enum MuxOp {
    /// Capture a tmux pane and stash the result in the App's
    /// preview store. Reducer-triggered
    /// after selection changes to a new mux target; the executor
    /// runs `capture-pane`, wraps the payload, and dispatches
    /// `Msg::SetPreview` back into the reducer.
    CapturePreview {
        mux: MuxSessionId,
        native_id: String,
    },
}

/// A durable-store write the executor should perform. Each variant
/// carries the fully-resolved request the pure resolver already
/// built; the executor writes the TOML, schedules a follow-up
/// refresh so the row tree reflects the change, and posts the
/// summary status.
#[derive(Debug, Clone, PartialEq)]
pub enum StoreOp {
    /// Pin a history record as a slot (`pin add <record id>`).
    PinRecord { record_id: String },
    /// Pin a graph node as a slot (`pin add <node id>`).
    PinNode { node: crate::model::NodeId },
    /// Remove a slot (`pin rm`).
    Unpin { slot: String },
    /// Rename a slot, and its mux when it is live (`pin rename`).
    RenameSlot { slot: String, new_name: String },
    /// Save the pin edit form: set and drop overrides, then rename.
    SavePin {
        request: crate::tui::widgets::pin_edit::PinEditRequest,
    },
    /// Point a slot at another entity (`pin retarget`).
    RetargetSlot { slot: String, target: String },
    /// Copy the entity's current recipe into the snapshot (`pin refresh`).
    RefreshSlot { slot: String },
    /// Undo of a pin: remove the pin with this name.
    RemovePin { id: String },
    /// Undo of an unpin: put the pin back at its old place in the order.
    RestorePin {
        slot: crate::model::PinSlot,
        index: usize,
    },
    /// Remove one history record.
    ForgetRecord { record_id: String },
    /// Remove these history records (the Clean up modal).
    ReapRecords { ids: Vec<String> },
    /// Commit the rename-overlay's confirm on an agent-session
    /// row (ADR 0029). The executor plans the rename against the
    /// held snapshot, writes the alias entry (or removes it when
    /// the operator cleared the field), and — when the plan
    /// carries a native mux rename — chains a tmux rename so the
    /// live session's title tracks the alias in lockstep.
    CommitAliasRename {
        session_id: AgentSessionId,
        new_display_name: Option<String>,
    },
    /// Commit the rename-overlay's confirm on a mux-session row.
    /// The executor plans the rename against the held snapshot,
    /// rewrites any pin whose `mux.name` matches the mux's
    /// current native id (graph-aware cascade), and chains a tmux
    /// `rename-session` so the live tmux name tracks the pin's
    /// intent in lockstep. Default socket only.
    CommitMuxRename {
        mux_id: MuxSessionId,
        new_name: String,
    },
    /// Create a worktree via the configured mutation backend
    /// (ADR 0092). The executor resolves the backend,
    /// runs `create`, toasts the outcome, and refreshes.
    WorktreeCreate { repo_root: String, branch: String },
    /// Remove a worktree via the configured mutation backend.
    /// `force` is set when the operator confirmed
    /// removal past the live-session guard.
    WorktreeRemove {
        repo_root: String,
        branch: String,
        force: bool,
    },
    /// Merge a worktree's branch back and tear it down via the
    /// configured mutation backend.
    WorktreeMerge {
        worktree_root: String,
        target: Option<String>,
    },
    /// Close down a whole stream of work (ADR 0093): the
    /// executor rebuilds the close-down plan from the held snapshot,
    /// terminates the worktree's mux sessions (graceful->hard),
    /// merges (`discard == false`) or removes (`discard == true`) the
    /// worktree, and forgets its history records (except ones a pin
    /// slot targets).
    WorktreeCloseDown {
        repo_root: String,
        branch: String,
        discard: bool,
    },
    /// Prune worktrees merged into the default branch via the
    /// configured mutation backend.
    WorktreePrune { repo_root: String },
}

/// What the runtime does around an effect so what the operator sees
/// stays truthful: whether it hands the terminal to a mux, freezes the
/// acted-on row until fresh data lands, refreshes afterwards, and moves
/// focus to wherever the entity now lives.
///
/// Every state-changing effect declares its lifecycle here, in one
/// place, so a new effect cannot ship without deciding. The match is
/// exhaustive on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lifecycle {
    /// The terminal is handed to a mux client.
    pub attaches: bool,
    /// The acted-on row is dimmed with a spinner until the refresh
    /// after the action lands.
    pub freezes_row: bool,
    /// A refresh runs once the action finishes.
    pub refreshes: bool,
    /// Focus moves to the view where the entity now lives, with its row
    /// selected.
    pub follows_entity: bool,
}

impl Lifecycle {
    /// An effect that changes nothing the views show.
    pub const NONE: Self = Self {
        attaches: false,
        freezes_row: false,
        refreshes: false,
        follows_entity: false,
    };

    /// A write that the views must catch up with.
    pub const REFRESHES: Self = Self {
        refreshes: true,
        ..Self::NONE
    };
}

/// What a refresh rescans before the screen is rebuilt. The rescan is
/// forced: the mux listing is asked of the server again and a running
/// daemon is told to re-probe, because the point of the refresh is to
/// see what the action just did, not what was cached a few seconds ago.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshRequest {
    /// Provider classes to re-probe, in order.
    pub classes: &'static [&'static str],
}

impl RefreshRequest {
    /// The classes whose state changes while the operator works in a
    /// mux: sessions appear, die, and get renamed, and harnesses start
    /// new transcripts.
    pub const LIVE: Self = Self {
        classes: &["mux", "harness"],
    };

    /// [`Self::LIVE`] plus the forges, whose listings are otherwise
    /// reused for the forge interval.
    pub const LIVE_AND_FORGE: Self = Self {
        classes: &["mux", "harness", "forge"],
    };

    /// Whether the mux listing must be asked of the server again.
    pub fn rescans_mux(&self) -> bool {
        self.classes.contains(&"mux")
    }
}

impl Lifecycle {
    /// The rescan the refresh after this effect runs, if it refreshes.
    pub fn refresh_request(&self) -> Option<RefreshRequest> {
        self.refreshes.then_some(RefreshRequest::LIVE)
    }
}

/// The lifecycle `effect` runs with today.
pub fn lifecycle(effect: &Effect) -> Lifecycle {
    match effect {
        Effect::SpawnRefresh { .. }
        | Effect::Toast(_)
        | Effect::Persist
        | Effect::Quit
        | Effect::RunMux(_) => Lifecycle::NONE,
        Effect::Exec(spec) => match spec {
            ExecSpec::AttachMux(_) => Lifecycle {
                attaches: true,
                freezes_row: true,
                refreshes: true,
                follows_entity: false,
            },
            ExecSpec::Resume(_)
            | ExecSpec::ViewSession(..)
            | ExecSpec::PaneScrollback { .. }
            | ExecSpec::ShowRebuildPlan { .. } => Lifecycle::NONE,
            ExecSpec::MuxNew { .. } | ExecSpec::MuxLaunch { .. } => Lifecycle {
                attaches: true,
                refreshes: true,
                follows_entity: true,
                ..Lifecycle::NONE
            },
            // The entity leaves History or the Pinned view's gone rows
            // and shows up live elsewhere, so the row freezes while it
            // comes back and focus follows it.
            ExecSpec::Rebuild { attach, .. } => Lifecycle {
                attaches: *attach,
                freezes_row: true,
                refreshes: true,
                follows_entity: true,
            },
        },
        Effect::WriteStore(op) => match op {
            StoreOp::PinRecord { .. }
            | StoreOp::PinNode { .. }
            | StoreOp::Unpin { .. }
            | StoreOp::RemovePin { .. }
            | StoreOp::RestorePin { .. }
            | StoreOp::RenameSlot { .. }
            | StoreOp::SavePin { .. }
            | StoreOp::RetargetSlot { .. }
            | StoreOp::RefreshSlot { .. }
            | StoreOp::ForgetRecord { .. }
            | StoreOp::ReapRecords { .. }
            | StoreOp::CommitAliasRename { .. }
            | StoreOp::CommitMuxRename { .. }
            | StoreOp::WorktreeCreate { .. }
            | StoreOp::WorktreeRemove { .. }
            | StoreOp::WorktreeMerge { .. }
            | StoreOp::WorktreeCloseDown { .. }
            | StoreOp::WorktreePrune { .. } => Lifecycle::REFRESHES,
        },
    }
}

/// Whether a store write can be undone, and why not when it can't.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoClass {
    /// The executor records the inverse the named [`crate::tui::undo::Undo`] carries.
    Undoable(&'static str),
    /// There is no inverse to record, for this reason.
    NotUndoable(&'static str),
}

/// Classify every store write for undo. The match has no wildcard arm:
/// a new [`StoreOp`] fails to compile until it says whether it can be
/// reversed.
pub fn undo_class(op: &StoreOp) -> UndoClass {
    use UndoClass::{NotUndoable, Undoable};
    match op {
        StoreOp::PinRecord { .. } | StoreOp::PinNode { .. } => Undoable("Undo::Unpin"),
        StoreOp::Unpin { .. } => Undoable("Undo::RestorePin"),
        StoreOp::RemovePin { .. } | StoreOp::RestorePin { .. } => {
            NotUndoable("it is an undo, and undoing it again would only flip the pin back")
        }
        StoreOp::RenameSlot { .. } => NotUndoable("renaming also renames a live tmux session"),
        StoreOp::SavePin { .. } => NotUndoable("the edit form can rename and change many fields"),
        StoreOp::RetargetSlot { .. } => NotUndoable("not yet recorded"),
        StoreOp::RefreshSlot { .. } => NotUndoable("not yet recorded"),
        StoreOp::ForgetRecord { .. } | StoreOp::ReapRecords { .. } => {
            NotUndoable("forgotten history records are not kept")
        }
        StoreOp::CommitAliasRename { .. } => NotUndoable("not yet recorded"),
        StoreOp::CommitMuxRename { .. } => NotUndoable("it renames a live tmux session"),
        StoreOp::WorktreeCreate { .. }
        | StoreOp::WorktreeRemove { .. }
        | StoreOp::WorktreeMerge { .. }
        | StoreOp::WorktreeCloseDown { .. }
        | StoreOp::WorktreePrune { .. } => NotUndoable("it changes git state outside Conspectus"),
    }
}
