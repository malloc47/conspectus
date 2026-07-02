//! Effects (ADR 0085 contract 2): data values the reducer emits
//! to schedule side effects. The runtime owns the effect executor,
//! which is the only code that touches `&mut Terminal`, the mux
//! runner, `std::process`, or the filesystem. `App::update` returns
//! `Vec<Effect>`; the reducer never blocks on I/O.
//!
//! The catalog grows one wave at a time. Reserved variants stay
//! out of the enum until the executor learns to run them so the
//! reducer never emits an effect nobody can execute. See
//! `docs/backlog.md` §H-TUI-002 for the phase roadmap.

use crate::tui::actions::AttachTarget;
use crate::tui::resume::ResumeTarget;

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
    /// Persist the current TUI state to disk (F8-013). Best-effort;
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
}

/// Description of a subprocess the executor should run. All variants
/// carry the resolved target data so the reducer never touches
/// `std::process` or the terminal.
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
}
