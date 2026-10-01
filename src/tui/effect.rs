//! Effects (ADR 0085 contract 2): data values the reducer emits
//! to schedule side effects. The runtime owns the effect executor,
//! which is the only code that touches `&mut Terminal`, the mux
//! runner, `std::process`, or the filesystem. `App::update` returns
//! `Vec<Effect>`; the reducer never blocks on I/O.
//!
//! Variants exist only once the executor can run them, so the
//! reducer never emits an effect nobody can execute.

use crate::model::{AgentSessionId, MuxSessionId};
use crate::tui::actions::{AttachTarget, PinLaunchTarget};
use crate::tui::resume::ResumeTarget;
use crate::tui::widgets::mux_launch::MuxLaunchRequest;
use crate::tui::widgets::pins::{
    PinBindRequest, PinCreateRequest, PinEditRequest, PinRemoveRequest,
};

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
    /// reducer never touches them.
    ViewSession(AgentSessionId),
    /// Suspend the alt screen, re-exec into
    /// `conspectus pin launch <id> --no-attach`, refresh so the row
    /// tree reflects the just-created session, then attach to the
    /// resulting tmux session when `attach_target` is available. The
    /// executor owns the full sequence; the reducer never sees the
    /// intermediate states.
    LaunchPin {
        pin_id: String,
        attach_target: Option<PinLaunchTarget>,
    },
    /// Suspend the alt screen, re-exec into
    /// `conspectus mux new <name> --cwd <cwd> --no-attach`, refresh
    /// so the row tree picks up the new session, then attach.
    /// Mirrors [`ExecSpec::LaunchPin`] but with no pin lookup —
    /// the operator supplied the fields via the form (ADR 0095).
    MuxNew { name: String, cwd: String },
    /// Suspend the alt screen, re-exec into
    /// `conspectus mux launch <harness> --name <name> …
    /// --no-attach`, refresh so the row tree picks up the new mux
    /// and its attributed harness session, then attach
    /// (ADR 0096). No pin is written.
    MuxLaunch { request: MuxLaunchRequest },
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
    /// `Msg::SetMuxPreview` back into the reducer.
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
    /// Write a new pin entry to its TOML store (ADR 0057). When
    /// `request.adopt_source_mux_name` is `Some`, the executor
    /// chains a tmux rename after the write so the just-adopted
    /// mux session takes the pin's declared name — see
    /// [`crate::tui::effect::MuxOp`]'s roadmap in
    /// `docs/backlog.md`.
    PinCreate(PinCreateRequest),
    /// Update an existing pin entry (id / display name / harness /
    /// cwd / mux / launch argv).
    PinEdit(PinEditRequest),
    /// Remove a pin entry from its TOML store (ADR 0057).
    PinRemove(PinRemoveRequest),
    /// Write a pin-binding declaration linking a pin to an
    /// existing agent session (ADR 0057 / ADR 0058).
    PinBind(PinBindRequest),
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
    /// worktree, and drops its pins.
    WorktreeCloseDown {
        repo_root: String,
        branch: String,
        discard: bool,
    },
    /// Prune worktrees merged into the default branch via the
    /// configured mutation backend.
    WorktreePrune { repo_root: String },
}
